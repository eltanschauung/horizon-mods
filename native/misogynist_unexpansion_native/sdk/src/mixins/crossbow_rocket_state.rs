// The crossbow rocket going off: who shot it (`None` the host, `Some(conn)` a LAN guest), if a player
// did.
pub static SHOOTER: std::sync::Mutex<Option<Option<u32>>> = std::sync::Mutex::new(None);
