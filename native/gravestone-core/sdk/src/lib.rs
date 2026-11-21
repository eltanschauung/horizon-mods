use mod_api::*;

#[derive(Default)]
struct Gravestone;

#[mod_api::horizon_mod]
impl Hooks for Gravestone {
    fn register(&self, r: &mut dyn Registrar, _: &RegistrationContext) {
        r.payload_handlers().registrar("1").play_raw(PayloadId("gravestone_core:waypoints".into()), None);
    }

    // Enable the engine's collected living-drop path. The native wrapper
    // saves the final list after the ordinary living-drops listeners ran.
    fn living_drops(&self, _: &mut dyn ServerHost, _: &mut LivingDropsEvent<'_>) -> Flow {
        Flow::Continue
    }
}
