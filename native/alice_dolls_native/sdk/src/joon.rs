//! Joon: the trading doll. Her owner sets her trades, as a villager's (something for something,
//! either side may be left empty for a gift), fills her with the stock to pay them out of, and
//! sends her to a player. She shows up at their side with a trade offer, follows them for 200
//! seconds, can't be hurt while she is out, and comes home when the time is up or they send her
//! back; she isn't sent to the same player again for 300 seconds. What she is paid goes into her
//! own inventory.
//!
//! Her screen is the mod's own (`joon_ui`); what it does on the server is `joon_play`.

use crate::stacks::DollStack as ItemStack;
use mod_api::Tag;

/// The most trades she holds: two columns of six.
pub const MAX_TRADES: usize = 12;
/// How long a visit lasts.
pub const VISIT_TICKS: u32 = 200 * 20;
/// How long before she is sent to the same player again, after a visit to them.
pub const COOLDOWN_SECS: u64 = 300;
/// Her storage: a large chest.
pub const SLOTS: usize = 54;

/// One trade: what the visited player pays (up to two stacks) and what they get.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Trade {
    pub cost: [Option<ItemStack>; 2],
    pub result: Option<ItemStack>,
}

impl Trade {
    pub fn is_set(&self) -> bool {
        self.result.is_some() || self.cost.iter().any(Option::is_some)
    }

    /// What the player gets, or, for a trade that only takes, the first thing it takes: what the
    /// list of trades shows for it.
    pub fn face(&self) -> Option<&ItemStack> {
        self.result.as_ref().or(self.cost[0].as_ref()).or(self.cost[1].as_ref())
    }
}

/// The same item: id, wear and components, whatever the count.
pub fn same(a: &ItemStack, b: &ItemStack) -> bool {
    a.item == b.item && a.damage == b.damage && a.extra == b.extra
}

/// How many of `want` are in `items`.
pub fn count_in(items: &[Option<ItemStack>], want: &ItemStack) -> u32 {
    items.iter().flatten().filter(|s| same(s, want)).map(|s| s.count).sum()
}

/// Takes `n` of `want` out of `items`, from the last slot back; all or nothing.
pub fn take_from(items: &mut [Option<ItemStack>], want: &ItemStack, n: u32) -> bool {
    if count_in(items, want) < n {
        return false;
    }
    let mut left = n;
    for slot in items.iter_mut().rev() {
        if left == 0 {
            break;
        }
        let Some(s) = slot else { continue };
        if !same(s, want) {
            continue;
        }
        let k = s.count.min(left);
        s.count -= k;
        left -= k;
        if s.count == 0 {
            *slot = None;
        }
    }
    true
}

/// Puts `stack` into `items`, onto stacks like it (up to `max` a slot) and then into empty slots;
/// what doesn't fit comes back.
pub fn put_into(items: &mut [Option<ItemStack>], mut stack: ItemStack, max: u32) -> Option<ItemStack> {
    let max = max.max(1);
    for slot in items.iter_mut().flatten() {
        if stack.count == 0 {
            return None;
        }
        if same(slot, &stack) && slot.count < max {
            let k = (max - slot.count).min(stack.count);
            slot.count += k;
            stack.count -= k;
        }
    }
    for slot in items.iter_mut() {
        if stack.count == 0 {
            return None;
        }
        if slot.is_none() {
            let mut s = stack.clone();
            s.count = stack.count.min(max);
            stack.count -= s.count;
            *slot = Some(s);
        }
    }
    (stack.count > 0).then_some(stack)
}

/// Whether her stock pays `trade` out once.
pub fn in_stock(items: &[Option<ItemStack>], trade: &Trade) -> bool {
    trade.result.as_ref().is_none_or(|r| count_in(items, r) >= r.count)
}

/// How many times her stock pays `trade` out.
pub fn stock_for(items: &[Option<ItemStack>], trade: &Trade) -> Option<u32> {
    let r = trade.result.as_ref()?;
    Some(count_in(items, r) / r.count.max(1))
}

/// Her side of one trade: what it pays out comes out of `items` and what it takes goes in. `max`
/// is an item's stack size. Unchanged, and Err with why, when it can't be done.
pub fn joon_side(items: &mut [Option<ItemStack>], trade: &Trade, max: impl Fn(&str) -> u32) -> Result<(), &'static str> {
    let mut after = items.to_vec();
    if let Some(r) = &trade.result
        && !take_from(&mut after, r, r.count)
    {
        return Err("out of stock");
    }
    for c in trade.cost.iter().flatten() {
        if put_into(&mut after, c.clone(), max(&c.item)).is_some() {
            return Err("full");
        }
    }
    items.clone_from_slice(&after);
    Ok(())
}

/// Whether `have` (the player's inventory) holds what `trade` takes.
pub fn can_pay(have: &[Option<ItemStack>], trade: &Trade) -> bool {
    // Two costs of the same item take from the same stacks.
    let mut need: Vec<ItemStack> = Vec::new();
    for c in trade.cost.iter().flatten() {
        match need.iter_mut().find(|n| same(n, c)) {
            Some(n) => n.count += c.count,
            None => need.push(c.clone()),
        }
    }
    need.iter().all(|n| count_in(have, n) >= n.count)
}

// ---- Saving ----

/// Her trades as saved with her: a list of compounds, each its three stacks by slot (0 and 1 what
/// it takes, 2 what it gives).
pub fn encode_trades(trades: &[Trade]) -> Tag {
    Tag::List(
        trades
            .iter()
            .map(|t| {
                let slots = vec![t.cost[0].clone(), t.cost[1].clone(), t.result.clone()];
                Tag::Compound([("Items".into(), crate::stacks::encode_items(&slots))].into())
            })
            .collect(),
    )
}

pub fn decode_trades(tag: Option<&Tag>) -> Vec<Trade> {
    let Some(Tag::List(list)) = tag else { return Vec::new() };
    list.iter()
        .take(MAX_TRADES)
        .map(|t| {
            let mut s = crate::stacks::decode_items(t.get("Items"), 3);
            s.resize(3, None);
            Trade { cost: [s[0].take(), s[1].take()], result: s[2].take() }
        })
        .collect()
}

/// Where a doll item keeps her trades, in its container: three slots a trade, past its baubles and
/// before its favour.
pub const TRADES_IN_ITEM: usize = 210;

/// Her trades, as her item carries them.
pub fn trades_to_item(trades: &[Trade]) -> Vec<(u8, ItemStack)> {
    let mut out = Vec::new();
    for (i, t) in trades.iter().take(MAX_TRADES).enumerate() {
        for (k, s) in [&t.cost[0], &t.cost[1], &t.result].into_iter().enumerate() {
            if let Some(s) = s {
                out.push(((TRADES_IN_ITEM + i * 3 + k) as u8, s.clone()));
            }
        }
    }
    out
}

pub fn trades_from_item(container: &[(u8, ItemStack)]) -> Vec<Trade> {
    let mut trades = vec![Trade::default(); MAX_TRADES];
    for (slot, s) in container {
        let Some(at) = usize::from(*slot).checked_sub(TRADES_IN_ITEM).filter(|&a| a < MAX_TRADES * 3) else { continue };
        let t = &mut trades[at / 3];
        match at % 3 {
            0 => t.cost[0] = Some(s.clone()),
            1 => t.cost[1] = Some(s.clone()),
            _ => t.result = Some(s.clone()),
        }
    }
    while trades.last().is_some_and(|t| !t.is_set()) {
        trades.pop();
    }
    trades
}

// ---- The screens ----

/// A player she can be sent to, as the list shows them.
#[derive(Clone, Debug, PartialEq)]
pub struct Visitee {
    pub uuid: u128,
    pub name: String,
    /// Seconds before she can go to them again; 0 now.
    pub wait: u32,
    pub you: bool,
}

/// Seconds as m:ss.
pub fn clock(secs: u32) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stacks::StackData;

    fn st(id: &str, n: u32) -> ItemStack {
        ItemStack::new(id, n)
    }

    fn emeralds_for_bread() -> Trade {
        Trade { cost: [Some(st("minecraft:emerald", 2)), None], result: Some(st("minecraft:bread", 6)) }
    }

    #[test]
    fn she_pays_out_of_her_stock_and_keeps_what_she_is_paid() {
        let mut stock = vec![None; SLOTS];
        stock[0] = Some(st("minecraft:bread", 10));
        let t = emeralds_for_bread();
        assert!(in_stock(&stock, &t));
        assert_eq!(stock_for(&stock, &t), Some(1));
        assert_eq!(joon_side(&mut stock, &t, |_| 64), Ok(()));
        assert_eq!(count_in(&stock, &st("minecraft:bread", 1)), 4);
        assert_eq!(count_in(&stock, &st("minecraft:emerald", 1)), 2);
        // Four loaves left: not enough for six. Nothing changes.
        let before = stock.clone();
        assert_eq!(joon_side(&mut stock, &t, |_| 64), Err("out of stock"));
        assert!(!in_stock(&stock, &t));
        assert_eq!(stock, before);
    }

    #[test]
    fn a_full_joon_takes_nothing() {
        let mut stock: Vec<Option<ItemStack>> = (0..SLOTS).map(|_| Some(st("minecraft:dirt", 64))).collect();
        stock[0] = Some(st("minecraft:bread", 6));
        let t = emeralds_for_bread();
        // The bread's slot empties, so the emeralds go there.
        assert_eq!(joon_side(&mut stock, &t, |_| 64), Ok(()));
        stock[0] = Some(st("minecraft:dirt", 64));
        stock[1] = Some(st("minecraft:bread", 12));
        let before = stock.clone();
        assert_eq!(joon_side(&mut stock, &t, |_| 64), Err("full"));
        assert_eq!(stock, before);
    }

    #[test]
    fn gifts_either_way() {
        let gift = Trade { cost: [None, None], result: Some(st("minecraft:cake", 1)) };
        let tip = Trade { cost: [Some(st("minecraft:emerald", 1)), None], result: None };
        assert!(gift.is_set() && tip.is_set() && !Trade::default().is_set());
        assert!(can_pay(&[], &gift));
        assert!(in_stock(&[], &tip));
        let mut stock = vec![None; 3];
        assert_eq!(joon_side(&mut stock, &tip, |_| 64), Ok(()));
        assert_eq!(stock[0], Some(st("minecraft:emerald", 1)));
        assert_eq!(tip.face(), Some(&st("minecraft:emerald", 1)));
    }

    #[test]
    fn the_player_pays_both_costs_of_one_item_from_the_same_stacks() {
        let t = Trade { cost: [Some(st("minecraft:emerald", 40)), Some(st("minecraft:emerald", 30))], result: None };
        assert!(!can_pay(&[Some(st("minecraft:emerald", 64))], &t));
        assert!(can_pay(&[Some(st("minecraft:emerald", 64)), Some(st("minecraft:emerald", 6))], &t));
    }

    #[test]
    fn an_item_is_the_same_only_with_the_same_components() {
        let mut named = st("minecraft:emerald", 1);
        named.extra = Some(Box::new(StackData { enchantments: vec![("minecraft:unbreaking".into(), 1)], ..StackData::default() }));
        assert!(!same(&named, &st("minecraft:emerald", 1)));
        assert_eq!(count_in(&[Some(named.clone())], &st("minecraft:emerald", 1)), 0);
    }

    #[test]
    fn her_trades_are_saved_with_her_and_in_her_item() {
        let trades = vec![emeralds_for_bread(), Trade::default(), Trade { cost: [None, Some(st("minecraft:diamond", 1))], result: None }];
        let back = decode_trades(Some(&encode_trades(&trades)));
        assert_eq!(back, trades);
        let in_item = trades_to_item(&trades);
        assert!(in_item.iter().all(|(s, _)| (210..250).contains(s)));
        assert_eq!(trades_from_item(&in_item), trades);
        assert_eq!(trades_from_item(&[]), Vec::new());
        assert_eq!(decode_trades(None), Vec::new());
    }

    #[test]
    fn a_clock_reads_minutes_and_seconds() {
        assert_eq!(clock(200), "3:20");
        assert_eq!(clock(5), "0:05");
    }
}
