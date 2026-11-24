//! Operator-only schematic uploads assembled on the server thread.
use mod_api::*;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

pub const LIMIT: usize = 8 * 1024 * 1024;
pub const PACKET: usize = 28_000;
const MAX_PENDING: usize = 8;

pub fn channel() -> PayloadId {
    PayloadId("worldedit:schematic".into())
}

pub struct Upload {
    pub paste: bool,
    pub data: Vec<u8>,
    started: Instant,
}

#[derive(Default)]
struct Transfers {
    pending: HashMap<PlayerRef, Upload>,
    complete: VecDeque<(PlayerRef, Upload)>,
}

thread_local! { static TRANSFERS: RefCell<Transfers> = RefCell::new(Transfers::default()); }

pub struct Receiver;
impl IPayloadHandler for Receiver {
    fn handle(&self, bytes: PayloadBytes, ctx: &mut dyn IPayloadContext) -> Result<(), PayloadHandleError> {
        let Ok(player) = ctx.player() else { return Ok(()) };
        let Ok(host) = ctx.server_host() else { return Ok(()) };
        if player.permission_level(host).unwrap_or(0) < 2 {
            return Ok(());
        }
        TRANSFERS.with_borrow_mut(|state| {
            state.pending.retain(|_, upload| upload.started.elapsed() < Duration::from_secs(30));
            match bytes.0.as_slice() {
                [mode @ (0 | 1)] if state.pending.contains_key(&player)
                    || state.pending.len() + state.complete.len() < MAX_PENDING => {
                    state.pending.insert(player, Upload {
                        paste: *mode == 1,
                        data: Vec::new(),
                        started: Instant::now(),
                    });
                }
                [2, data @ ..] if !data.is_empty() && data.len() <= PACKET => {
                    if let Some(upload) = state.pending.get_mut(&player) {
                        if upload.data.len() + data.len() <= LIMIT {
                            upload.data.extend_from_slice(data);
                        } else {
                            state.pending.remove(&player);
                        }
                    }
                }
                [3] => {
                    if let Some(upload) = state.pending.remove(&player)
                        && !upload.data.is_empty()
                    {
                        state.complete.push_back((player, upload));
                    }
                }
                _ => {}
            }
        });
        Ok(())
    }
}

pub fn take_upload() -> Option<(PlayerRef, Upload)> {
    TRANSFERS.with_borrow_mut(|state| state.complete.pop_front())
}

pub fn disconnected(uuid: u128) {
    TRANSFERS.with_borrow_mut(|state| {
        state.pending.retain(|player, _| player.uuid() != uuid);
        state.complete.retain(|(player, _)| player.uuid() != uuid);
    });
}

pub fn reset() {
    TRANSFERS.with_borrow_mut(|state| *state = Transfers::default());
}
