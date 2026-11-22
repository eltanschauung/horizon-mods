// The enchant click in progress: the level before it, the offer's cost and the slot's levels.
pub static CLICK: std::sync::Mutex<Option<(i32, i32, i32)>> = std::sync::Mutex::new(None);
