//! Block Variety's native part: the chisel, the one way between a group's blocks of Chipped and
//! Chisel (there are no Chipped workbenches), and Block Carpentry's frames (`frames`).
//!
//! - Right-click with the chisel: the chisel's screen (`chisel_ui`), every block of the group of
//!   what is in your other hand as icons, with a search. Click one, and that whole stack becomes
//!   it; right-click, and one block does.
//! - Right-click a placed block of a group with the chisel: it becomes the group's next block
//!   (sneaking, the one before), keeping its facing, connections and the rest of its state.
use mod_api::*;
use std::sync::Arc;

mod chisel_ui;
mod frames;
mod groups;
mod names;
mod trash;

use chisel_ui::{MOD_ID, UI, Ui};

#[derive(Default)]
struct Mod;

const CHISEL: &str = "block_variety:chisel";
/// The screen's sound and the chisel's, on a block.
const CLICK: &str = "minecraft:ui.button.click";
const CHIP: &str = "minecraft:block.stone.hit";
/// The group `block` is in, if any.
fn group_of(block: &str) -> Option<&'static [&'static str]> {
    groups::GROUPS.iter().copied().find(|g| g.contains(&block))
}

/// The block after `block` in its group (before it when `back`), round to the start.
fn step(block: &str, back: bool) -> Option<&'static str> {
    let g = group_of(block)?;
    let i = g.iter().position(|b| *b == block)?;
    let n = g.len();
    Some(g[if back { (i + n - 1) % n } else { (i + 1) % n }])
}

/// What the menu offers of a group: its blocks that have an item.
fn entries(group: &'static [&'static str]) -> Vec<&'static str> {
    group
        .iter()
        .copied()
        .filter(|b| !groups::ITEMLESS.contains(b))
        .collect()
}

pub fn item_key(host: &dyn Host, id: &str) -> Option<ItemKey> {
    host.item_key(&Identifier::parse_namespaced(id)?)
}

fn held(host: &mut dyn ServerHost, player: PlayerRef, hand: Hand) -> Option<ItemStack> {
    ItemPlace::PlayerHand {
        player: player.uuid(),
        hand,
    }
    .stack(host)
}

fn play(host: &mut dyn ServerHost, player: PlayerRef, sound: &str) {
    if let (Some(level), Some(at)) = (player.level(host), player.entity().position(host)) {
        level.play_sound(host, sound, at, SoundSource::Blocks, 0.6, 1.0);
    }
}

fn show(host: &mut dyn ServerHost, player: PlayerRef) {
    let stack = held(host, player, Hand::Off);
    let (id, count) = stack
        .as_ref()
        .map(|stack| (stack.item().id(host), stack.count()))
        .filter(|(id, _)| group_of(id).is_some())
        .unwrap_or_default();
    let _ = player.send_payload(
        host,
        &PayloadId(chisel_ui::OPEN.into()),
        PayloadBytes(chisel_ui::encode_open(&id, count)),
    );
}

fn chisel(host: &mut dyn ServerHost, player: PlayerRef, target: &str, whole: bool) {
    if !held(host, player, Hand::Main).is_some_and(|stack| stack.item().id(host) == CHISEL) {
        return;
    }
    let Some(stack) = held(host, player, Hand::Off) else {
        return;
    };
    let id = stack.item().id(host);
    let Some(group) = group_of(&id) else { return };
    if id == target || !group.contains(&target) || groups::ITEMLESS.contains(&target) {
        return;
    }
    let Some(key) = item_key(host, target) else {
        return;
    };
    let split = !whole && stack.count() > 1;
    let changed = host
        .transaction(|host, _| -> Result<(), ()> {
            if split {
                player
                    .set_inventory_slot(host, 40, stack.clone().shrink(1))
                    .map_err(|_| ())?;
                player
                    .give_stack(host, ItemStack::of(key))
                    .map_err(|_| ())?;
            } else {
                let next = ItemStack::new(key, stack.count()).ok_or(())?;
                player
                    .set_inventory_slot(host, 40, Some(next))
                    .map_err(|_| ())?;
            }
            Ok(())
        })
        .is_ok();
    if changed {
        play(host, player, CHIP);
    }
}

struct Frame;
#[mod_api::horizon_part]
impl BlockPart for Frame {}
#[mod_api::horizon_part]
impl BlockEntityPart for Frame {
    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks |= frames::ticks(event.block);
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        frames::tick(host, event);
    }

    fn block_entity_update_tag(
        &self,
        event: &mut BlockEntityUpdateTagEvent<'_>,
    ) -> Option<Compound> {
        frames::update_tag(event)
    }

    fn block_entity_submit(&self, host: &dyn ClientHost, event: &mut BlockEntitySubmitEvent<'_>) {
        frames::submit(host, event);
    }
}

struct Trash;
#[mod_api::horizon_part]
impl BlockPart for Trash {}
#[mod_api::horizon_part]
impl BlockEntityPart for Trash {
    fn block_entity_ticks(&self, _: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks |= trash::ticks(event.block);
    }

    fn block_entity_tick(&self, _: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        trash::tick(event);
    }
}

struct Choose;
impl IPayloadHandler for Choose {
    fn handle(
        &self,
        bytes: PayloadBytes,
        ctx: &mut dyn IPayloadContext,
    ) -> Result<(), PayloadHandleError> {
        if bytes.0.len() <= 300
            && let Some((target, whole)) = chisel_ui::decode_choose(&bytes.0)
            && let Ok(player) = ctx.player()
            && let Ok(host) = ctx.server_host()
        {
            chisel(host, player, &target, whole);
            show(host, player);
        }
        Ok(())
    }
}

struct Open;
impl IPayloadHandler for Open {
    fn handle(
        &self,
        bytes: PayloadBytes,
        ctx: &mut dyn IPayloadContext,
    ) -> Result<(), PayloadHandleError> {
        let (Some((held, count)), Ok(host)) = (chisel_ui::decode_open(&bytes.0), ctx.client_host())
        else {
            return Ok(());
        };
        if !chisel_ui::set_held(held, count) {
            host.mod_screen_close(MOD_ID);
        } else if host.mod_screen() != Some(MOD_ID)
            && !host.mod_screen_open(MOD_ID)
            && let Ok(mut ui) = UI.lock()
        {
            *ui = Ui::Closed;
        }
        Ok(())
    }
}

/// Whether the screen open is the chisel's.
fn ours(host: &dyn ClientHost, id: &str) -> bool {
    id == "mod" && host.mod_screen() == Some(MOD_ID)
}

fn blink() -> bool {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    ms / 500 % 2 == 0
}

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        let frame = r.block_entity_type("frame").blocks(groups::FRAMES).build();
        let trash = r
            .block_entity_type("item_trash_can")
            .blocks(&[trash::BLOCK])
            .build();
        let _ = r.block_entity_part(&frame, Frame);
        let _ = r.block_entity_part(&trash, Trash);
        for &block in groups::FRAMES.iter().chain(std::iter::once(&trash::BLOCK)) {
            if let Some(container) = frames::container(block).or_else(|| trash::container(block)) {
                let key = r.block_key(block);
                r.block_container(&key, container);
            }
        }
        r.payload_handlers()
            .registrar("1")
            .optional()
            .play_to_server(PayloadId(chisel_ui::CHOOSE.into()), Arc::new(Choose))
            .play_to_client(PayloadId(chisel_ui::OPEN.into()), Some(Arc::new(Open)));
    }

    fn break_block(&self, host: &mut dyn ServerHost, event: &mut BreakBlockEvent<'_>) -> Flow {
        if host
            .block_state_string(event.state)
            .is_some_and(|state| frames::is_frame(state.split('[').next().unwrap_or(&state)))
        {
            frames::destroy_block(host, event);
        }
        Flow::Continue
    }

    fn right_click_block(&self, host: HostSide, event: &mut RightClickBlockEvent<'_>) -> Flow {
        let HostSide::Server(host) = host else {
            return Flow::Continue;
        };
        let click = event.click;
        let Some(player) = host.player(click.player) else {
            return Flow::Continue;
        };
        let Some(level) = player.level(host) else {
            return Flow::Continue;
        };
        let Some(state) = level
            .block_state(host, click.pos)
            .and_then(|state| host.block_state_string(state))
        else {
            return Flow::Continue;
        };
        let (block, props) = match state.find('[') {
            Some(i) => state.split_at(i),
            None => (state.as_str(), ""),
        };
        let chisel = click
            .stack
            .is_some_and(|stack| stack.item().id(host) == CHISEL);
        let used = if chisel {
            if let Some(next) = step(block, player.is_secondary_use_active(host).unwrap_or(false))
                && let Some(state) = host
                    .block_state_id(&format!("{next}{props}"))
                    .or_else(|| host.block_state_id(next))
                && level
                    .set_block(
                        host,
                        click.pos,
                        state,
                        BlockUpdateFlags::CLIENTS | BlockUpdateFlags::NEIGHBORS,
                    )
                    .unwrap_or(false)
            {
                level.play_sound(
                    host,
                    CHIP,
                    click.pos.map(|x| f64::from(x) + 0.5),
                    SoundSource::Blocks,
                    0.6,
                    1.0,
                );
                true
            } else {
                false
            }
        } else {
            frames::is_frame(block) && frames::click(host, player, level, &state, click)
        };
        if used {
            *event.cancellation_result = InteractionResult::SuccessServer;
            Flow::Cancel
        } else {
            Flow::Continue
        }
    }

    fn right_click_item(&self, host: HostSide, event: &mut RightClickItemEvent<'_>) -> Flow {
        let HostSide::Server(host) = host else {
            return Flow::Continue;
        };
        if event.click.hand != Hand::Main || event.click.stack.item().id(host) != CHISEL {
            return Flow::Continue;
        }
        let Some(player) = host.player(event.click.player) else {
            return Flow::Continue;
        };
        let stack = held(host, player, Hand::Off);
        if stack
            .as_ref()
            .and_then(|stack| group_of(&stack.item().id(host)))
            .is_none_or(|group| entries(group).len() < 2)
        {
            let _ = player.action_bar(
                host,
                Text::literal("Chisel: hold the blocks to chisel in your other hand"),
            );
        } else {
            show(host, player);
            play(host, player, CLICK);
        }
        *event.cancellation_result = InteractionResult::Consume;
        Flow::Cancel
    }

    /// Sends what the screen chose, and forgets a screen Escape closed.
    fn client_tick_post(&self, host: &mut dyn ClientHost, _: &mut ClientTickPostEvent) {
        let out = chisel_ui::OUTBOX
            .lock()
            .map(|mut o| std::mem::take(&mut *o))
            .unwrap_or_default();
        for (channel, data) in out {
            let _ = host.send_payload_to_server(&PayloadId(channel.into()), PayloadBytes(data));
        }
        if host.mod_screen() != Some(MOD_ID)
            && let Ok(mut ui) = UI.lock()
        {
            *ui = Ui::Closed;
        }
    }

    fn screen_render_post(&self, host: &dyn ClientHost, event: &mut ScreenRenderPostEvent<'_>) {
        if ours(host, event.screen.id) {
            chisel_ui::draw(
                host,
                event.draw,
                event.screen.rect,
                event.screen.mouse,
                blink(),
            );
        }
    }

    fn screen_char_typed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenCharTypedPreEvent<'_>,
    ) -> Flow {
        if !ours(host, event.screen.id) {
            return Flow::Continue;
        }
        chisel_ui::typed(event.c);
        Flow::Cancel
    }

    fn screen_key_pressed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenKeyPressedPreEvent<'_>,
    ) -> Flow {
        if !ours(host, event.screen.id) || event.key == "key.keyboard.escape" {
            return Flow::Continue;
        }
        chisel_ui::key(event.key);
        Flow::Cancel
    }

    fn screen_mouse_button_pressed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseButtonPressedPreEvent<'_>,
    ) -> Flow {
        if !ours(host, event.screen.id) {
            return Flow::Continue;
        }
        chisel_ui::pressed(event.screen.rect, event.screen.mouse, event.button);
        Flow::Cancel
    }

    fn screen_mouse_button_released_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseButtonReleasedPreEvent<'_>,
    ) -> Flow {
        if !ours(host, event.screen.id) {
            return Flow::Continue;
        }
        chisel_ui::released();
        Flow::Cancel
    }

    fn screen_mouse_dragged_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseDraggedPreEvent<'_>,
    ) -> Flow {
        if !ours(host, event.screen.id) {
            return Flow::Continue;
        }
        chisel_ui::dragged(event.screen.rect, event.screen.mouse);
        Flow::Cancel
    }

    fn screen_mouse_scrolled_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseScrolledPreEvent<'_>,
    ) -> Flow {
        if !ours(host, event.screen.id) {
            return Flow::Continue;
        }
        chisel_ui::scrolled(event.screen.rect, event.scroll[1]);
        Flow::Cancel
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obsidian_variants_go_round() {
        let g = group_of("minecraft:obsidian").expect("obsidian is a group");
        assert!(g.len() > 30, "{}", g.len());
        assert_eq!(g[0], "minecraft:obsidian");
        let last = g[g.len() - 1];
        assert_eq!(step(last, false), Some("minecraft:obsidian"));
        assert_eq!(step("minecraft:obsidian", true), Some(last));
        assert_eq!(
            step("block_variety:obsidian_1", false).map(|b| b != "block_variety:obsidian_1"),
            Some(true)
        );
    }

    #[test]
    fn every_block_is_in_one_group() {
        let mut seen = std::collections::HashSet::new();
        for g in groups::GROUPS {
            assert!(g.len() > 1 || !g[0].starts_with("minecraft:"), "{g:?}");
            for b in *g {
                assert!(seen.insert(*b), "{b} is in two groups");
            }
        }
        assert!(
            group_of("minecraft:crying_obsidian")
                .unwrap()
                .iter()
                .all(|b| b.contains("crying_obsidian"))
        );
    }

    #[test]
    fn wall_torches_turn_but_are_not_offered() {
        let g = group_of("minecraft:wall_torch").expect("wall torches are a group");
        assert!(g.len() > 1);
        assert!(entries(g).is_empty());
        assert!(entries(group_of("minecraft:torch").unwrap()).len() > 1);
    }
}
