// SPDX-License-Identifier: MIT
fn classic_music_resolve(engine: &mut SoundEngine, id: InstanceId) {
    use crate::sound_events::classic_music_registry as registry;
    let Some(event) = engine.instance(id).map(|i| i.identifier.clone()) else {
        return;
    };
    let Some(group) = registry::prepare(&mut engine.registry, &event) else {
        return;
    };
    if let Some(instance) = engine.instance_mut(id)
        && instance.chosen
        && instance
            .sound
            .as_ref()
            .is_some_and(|s| !group.allows(&s.location))
    {
        instance.chosen = false;
        instance.sound = None;
        instance.event = None;
    }
}
