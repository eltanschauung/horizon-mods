//! Trash Cans' Item Trash Can (by SuperMartijn642): a row of nine slots, from a player's hand or a
//! hopper or anything that fills a container, emptied every tick. What goes in is gone.
use mod_api::*;

pub const BLOCK: &str = "block_variety:item_trash_can";
pub const ENTITY: &str = "block_variety_native:item_trash_can";
const SLOTS: usize = 9;

/// Nine slots that every face fills and none takes from.
pub fn container(block: &str) -> Option<ModContainer> {
    if block != BLOCK {
        return None;
    }
    let all: Vec<usize> = (0..SLOTS).collect();
    Some(ModContainer {
        slots: SLOTS,
        faces: std::array::from_fn(|_| all.clone()),
        inputs: all,
        outputs: Vec::new(),
        menu: Some("minecraft:generic_9x1".to_owned()),
    })
}

pub fn ticks(block: &str) -> bool {
    block == BLOCK
}

/// Empties the can (touching it only when there is something in it, so an empty can is not
/// counted as changed).
pub fn tick(event: &mut BlockEntityTickEvent<'_>) {
    if event.entity.id() != ENTITY || !has_items(&event.entity.items) {
        *event.reply = BlockEntityTickReply::Unchanged;
        return;
    }
    for slot in event.entity.items.iter_mut() {
        *slot = None;
    }
}

fn has_items<T>(items: &[Option<T>]) -> bool {
    items.iter().any(Option::is_some)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_can_takes_in_and_gives_nothing_out() {
        let c = container(BLOCK).unwrap();
        assert_eq!(c.slots, 9);
        assert_eq!(c.inputs.len(), 9);
        assert!(c.outputs.is_empty());
        assert!(c.faces.iter().all(|f| f.len() == 9));
        assert!(container("block_variety:frame_chest").is_none());
        assert!(ticks(BLOCK));
    }

    #[test]
    fn an_empty_can_is_left_alone() {
        assert!(!has_items::<u8>(&[None, None]));
        assert!(has_items(&[None, Some(1)]));
    }
}
