pub mod paths;
pub mod uploads;

use mod_api::*;
use std::sync::Arc;

#[derive(Default)]
struct WorldEdit;

#[mod_api::horizon_mod]
impl Hooks for WorldEdit {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        r.payload_handlers().registrar("1").optional().play_to_server(uploads::channel(), Arc::new(uploads::Receiver));
    }

    fn player_logged_out(&self, _: &mut dyn ServerHost, event: &mut PlayerLoggedOutEvent<'_>) {
        uploads::disconnected(event.player);
    }

    fn server_stopping(&self, _: &mut dyn ServerHost, _: &mut ServerStoppingEvent) {
        uploads::reset();
    }
}
