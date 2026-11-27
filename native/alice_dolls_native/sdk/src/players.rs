//! Scoped reads and inventory edits for the player currently using a doll.

use mod_api::{
    GameMode, Hand, Host, HostTransaction, ItemPlace, ItemResource, MenuReplace, PlayerRef,
    ServerHost, Text,
};

use crate::stacks::DollStack;

pub struct PlayerView {
    pub uuid: u128,
    pub entity_id: i32,
    pub position: [f64; 3],
    pub dimension: String,
    pub name: String,
    pub yaw: f32,
    pub pitch: f32,
    pub game_mode: i32,
    pub sneaking: bool,
    pub health: f32,
    pub eye_height: f32,
}

impl PlayerView {
    fn from_player(host: &dyn ServerHost, player: PlayerRef) -> Option<Self> {
        let entity = player.entity();
        let [yaw, pitch] = entity.rotation(host)?;
        let game_mode = match player.game_mode(host)? {
            GameMode::Creative => 1,
            GameMode::Adventure => 2,
            GameMode::Spectator => 3,
            _ => 0,
        };
        Some(Self {
            uuid: player.uuid(),
            entity_id: entity.id() as i32,
            position: entity.position(host)?,
            dimension: player.level(host)?.dimension_id().to_owned(),
            name: player.name(host)?,
            yaw,
            pitch,
            game_mode,
            sneaking: player.is_secondary_use_active(host) == Some(true),
            health: host.living(entity)?.health(host)?,
            eye_height: entity.eye_height(host)? as f32,
        })
    }
}

pub trait DollPlayerAccess {
    fn acting_player(&self) -> Option<PlayerView>;
    fn server_players(&self) -> Vec<PlayerView>;
    fn action_bar(&mut self, player: u128, text: Text);
    fn tell(&mut self, player: u128, text: Text);
    fn give_experience(&mut self, player: u128, points: u32);
    fn carried_stack(&mut self, slot: i32) -> Option<DollStack>;
    fn take_items(&mut self, item: &str, count: u32) -> u32;
    fn held_item(&mut self) -> Option<String>;
    fn give_doll_stack(&mut self, player: u128, stack: DollStack) -> Option<DollStack>;
    fn open_doll_view(&mut self, player: u128, rows: u8, title: Text, items: Vec<Option<DollStack>>) -> Option<i32>;
    fn update_doll_view(&mut self, player: u128, items: Vec<Option<DollStack>>);
    fn menu_contents(&self, player: u128) -> Option<Vec<Option<DollStack>>>;
    /// What `player` carries: their 36 slots and their off hand.
    fn player_stacks(&mut self, player: u128) -> Vec<Option<DollStack>>;
    /// Takes every one of `wants` (the same item, wear and components) out of `player`'s
    /// inventory: all of them, or, when they haven't got them all, nothing.
    fn take_stacks(&mut self, player: u128, wants: &[DollStack]) -> bool;
}

/// A player's slots, as `ItemPlace::PlayerSlot` counts them: 36, and the off hand.
fn carried_slots() -> impl Iterator<Item = i32> {
    (0..36).chain([-106])
}

impl DollPlayerAccess for dyn ServerHost + '_ {
    fn acting_player(&self) -> Option<PlayerView> {
        PlayerView::from_player(self, self.players().acting_player()?)
    }

    fn server_players(&self) -> Vec<PlayerView> {
        self.players()
            .server_players()
            .into_iter()
            .filter_map(|player| PlayerView::from_player(self, player))
            .collect()
    }

    fn action_bar(&mut self, player: u128, text: Text) {
        if let Some(player) = self.player(player) {
            let _ = player.action_bar(self, text);
        }
    }

    fn tell(&mut self, player: u128, text: Text) {
        if let Some(player) = self.player(player) {
            let _ = player.tell(self, text);
        }
    }

    fn give_experience(&mut self, player: u128, points: u32) {
        if let Some(player) = self.player(player) {
            player.give_experience_points(self, points);
        }
    }

    fn carried_stack(&mut self, slot: i32) -> Option<DollStack> {
        let player = self.players().acting_player()?.uuid();
        let stack = ItemPlace::PlayerSlot { player, slot }.stack(self)?;
        DollStack::from_game(self, &stack)
    }

    fn take_items(&mut self, item: &str, count: u32) -> u32 {
        let Some(player) = self.players().acting_player().map(PlayerRef::uuid) else { return 0 };
        self.transaction(|host, tx| {
            let mut taken = 0;
            for slot in (0..36).chain([-106]) {
                if taken == count { break; }
                let place = ItemPlace::PlayerSlot { player, slot };
                let Some(stack) = place.stack(host) else { continue };
                if stack.item().id(host) == item {
                    taken += place.extract(host, tx, &ItemResource::of(&stack), count - taken);
                }
            }
            Ok::<_, ()>(taken)
        }).unwrap_or(0)
    }
    fn held_item(&mut self) -> Option<String> {
        let player = self.players().acting_player()?.uuid();
        let place = ItemPlace::PlayerHand { player, hand: Hand::Main };
        Some(place.stack(self)?.item().id(self))
    }

    fn give_doll_stack(&mut self, player: u128, mut stack: DollStack) -> Option<DollStack> {
        if self.player(player).is_none() {
            return Some(stack);
        }
        let mut single = stack.clone();
        single.count = 1;
        let Some(single) = single.to_game(self) else {
            return Some(stack);
        };
        let resource = ItemResource::of(&single);
        let inserted = self.transaction(|host, tx| {
            let mut left = stack.count;
            for existing in [true, false] {
                for slot in 0..36 {
                    if left == 0 {
                        return Ok::<_, ()>(stack.count);
                    }
                    let place = ItemPlace::PlayerSlot { player, slot };
                    let current = place.stack(host);
                    let accepts = match current {
                        Some(current) => existing && resource.matches(&current),
                        None => !existing,
                    };
                    if accepts {
                        left -= place.insert(host, tx, &resource, left);
                    }
                }
            }
            Ok(stack.count - left)
        }).unwrap_or(0);
        stack.count -= inserted;
        (stack.count > 0).then_some(stack)
    }

    fn open_doll_view(&mut self, player: u128, rows: u8, title: Text, items: Vec<Option<DollStack>>) -> Option<i32> {
        let items = menu_items(self, items)?;
        self.open_menu(player, rows, title, items, MenuReplace::CloseFirst)
            .ok()
            .flatten()
    }

    fn update_doll_view(&mut self, player: u128, items: Vec<Option<DollStack>>) {
        if let Some((window, items)) = self.player(player)
            .and_then(|player| player.container_menu(self))
            .and_then(|menu| menu.container_id(self))
            .zip(menu_items(self, items))
        {
            let _ = self.menu_set_items(player, window, items);
        }
    }

    fn menu_contents(&self, player: u128) -> Option<Vec<Option<DollStack>>> {
        let menu = self.player(player)?.container_menu(self)?;
        let window = menu.container_id(self)?;
        let menu = self.menu(player, window)?;
        (0..menu.slot_count(self)?)
            .map(|slot| menu.slot(self, slot).map(|stack| stack.and_then(|stack| DollStack::from_game(self, &stack))))
            .collect()
    }

    fn player_stacks(&mut self, player: u128) -> Vec<Option<DollStack>> {
        if self.player(player).is_none() {
            return Vec::new();
        }
        let mut out = Vec::new();
        for slot in carried_slots() {
            let stack = ItemPlace::PlayerSlot { player, slot }.stack(self);
            out.push(stack.and_then(|s| DollStack::from_game(self, &s)));
        }
        out
    }

    fn take_stacks(&mut self, player: u128, wants: &[DollStack]) -> bool {
        if self.player(player).is_none() {
            return false;
        }
        self.transaction(|host, tx| {
            for want in wants {
                let mut left = want.count;
                for slot in carried_slots() {
                    if left == 0 {
                        break;
                    }
                    let place = ItemPlace::PlayerSlot { player, slot };
                    let Some(stack) = place.stack(host) else { continue };
                    if !DollStack::from_game(host, &stack).is_some_and(|s| crate::joon::same(&s, want)) {
                        continue;
                    }
                    left -= place.extract(host, tx, &ItemResource::of(&stack), left).min(left);
                }
                if left > 0 {
                    return Err(());
                }
            }
            Ok(())
        })
        .is_ok()
    }
}

fn menu_items(host: &dyn Host, items: Vec<Option<DollStack>>) -> Option<Vec<Option<mod_api::ItemStack>>> {
    items.into_iter().map(|item| match item {
        Some(stack) => stack.to_game(host).map(Some),
        None => Some(None),
    }).collect()
}


pub fn max_stack_size(host: &dyn Host, id: &str) -> Option<u32> {
    let key = host.item_key(&mod_api::Identifier::parse(id).ok()?)?;
    host.max_stack_size(key)
}
