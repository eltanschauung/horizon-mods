// SPDX-License-Identifier: MIT
use super::{Sound, SoundRegistry, WeighedSoundEvents, classic_music_policy as policy};
use std::collections::HashMap;
const ACTIVE: &str = "classic_music:active";
const ORIGINAL: &str = "classic_music:original/";
const POOL: &str = "classic_music:pool/";

/// Called once per client tick; the common path is a single registry lookup.
/// Snapshots live in the registry itself, so reloads/clones cannot retain stale pools.
pub fn sync(registry: &mut SoundRegistry, enabled: bool) -> bool {
    if enabled == registry.contains(ACTIVE) {
        return false;
    }
    if !enabled {
        let restore: Vec<_> = registry
            .events
            .iter()
            .filter_map(|(name, event)| {
                Some((
                    format!("minecraft:{}", name.strip_prefix(ORIGINAL)?),
                    event.clone(),
                ))
            })
            .collect();
        for (name, event) in restore {
            registry.events.insert(name, event);
        }
        registry.events.retain(|name, _| {
            name != ACTIVE && !name.starts_with(ORIGINAL) && !name.starts_with(POOL)
        });
        return true;
    }
    let mut names: Vec<_> = registry.events.keys().cloned().collect();
    names.sort();
    let mut files: HashMap<String, Sound> = HashMap::new();
    for name in &names {
        for sound in &registry.events[name].sounds {
            if !sound.event {
                files
                    .entry(sound.location.clone())
                    .or_insert_with(|| sound.clone());
            }
        }
    }
    for pool in policy::POOLS {
        let sounds = pool
            .tracks()
            .iter()
            .filter_map(|path| {
                let mut sound = files.get(&format!("minecraft:{path}"))?.clone();
                sound.weight = 1;
                sound.stream = true;
                Some(sound)
            })
            .collect();
        registry.events.insert(
            format!("{POOL}{}", pool.name()),
            WeighedSoundEvents {
                sounds,
                subtitle: None,
            },
        );
    }
    registry
        .events
        .insert(ACTIVE.to_owned(), WeighedSoundEvents::default());
    for name in names {
        prepare(registry, &name);
    }
    true
}

/// Also runs at resolution, before an audio channel opens. Later-added Minecraft
/// background events get the same playlists; discs never enter this branch.
pub fn prepare(registry: &mut SoundRegistry, event: &str) -> Option<policy::Pool> {
    let group = policy::group(event)?;
    if !registry.contains(ACTIVE) {
        return None;
    }
    let Some(original) = registry.events.get(event) else {
        return None;
    };
    let backup = format!("{ORIGINAL}{}", event.strip_prefix("minecraft:")?);
    if !registry.events.contains_key(&backup) {
        registry.events.insert(backup, original.clone());
    }
    let replacement = registry
        .events
        .get(&format!("{POOL}{}", group.name()))?
        .clone();
    let entry = registry.events.get_mut(event)?;
    if entry.sounds != replacement.sounds {
        entry.sounds = replacement.sounds;
    }
    Some(group)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn registry() -> SoundRegistry {
        let mut r = SoundRegistry::default();
        for pool in policy::POOLS {
            let sounds = pool
                .tracks()
                .iter()
                .map(|p| Sound::file(&format!("minecraft:{p}")))
                .collect();
            r.events.insert(
                format!("minecraft:music.{}", pool.name()),
                WeighedSoundEvents {
                    sounds,
                    subtitle: None,
                },
            );
        }
        r.add_file(
            "minecraft",
            br#"{
          "music.overworld.swamp":{"sounds":["music/game/swamp/aerie"]},
          "music.nether.nether_wastes":{"sounds":["music/game/nether/nether_wastes/rubedo"]},
          "music_disc.pigstep":{"sounds":["records/pigstep"]}
        }"#,
            &|_, _| true,
        );
        r
    }
    #[test]
    fn replacement_has_no_new_tracks_or_empty_biomes_and_restores_exactly() {
        let mut r = registry();
        let before = r.events.clone();
        assert!(sync(&mut r, true));
        assert!(!sync(&mut r, true));
        assert_eq!(r.tracks("minecraft:music.overworld.swamp").len(), 21);
        assert_eq!(r.tracks("minecraft:music.nether.nether_wastes").len(), 4);
        assert_eq!(r.tracks("minecraft:music.menu").len(), 4);
        assert_eq!(
            r.tracks("minecraft:music_disc.pigstep"),
            ["minecraft:records/pigstep"]
        );
        assert!(sync(&mut r, false));
        assert_eq!(r.events, before);
    }
    #[test]
    fn reload_clone_and_late_registration_stay_correct() {
        let mut r = registry();
        sync(&mut r, true);
        let mut cloned = r.clone();
        sync(&mut cloned, false);
        assert!(!cloned.contains(ACTIVE));
        r.add_file(
            "minecraft",
            br#"{"music.overworld.future":{"sounds":["music/game/new_song"]}}"#,
            &|_, _| true,
        );
        assert_eq!(
            prepare(&mut r, "minecraft:music.overworld.future"),
            Some(policy::Pool::Overworld)
        );
        assert_eq!(r.tracks("minecraft:music.overworld.future").len(), 21);
        let mut fresh = registry();
        assert!(sync(&mut fresh, true));
        assert_eq!(fresh.tracks("minecraft:music.menu").len(), 4);
    }
    #[test]
    fn sampled_tracks_use_real_client_assets_and_exact_playlists() {
        let Ok(root) = std::env::var("CLASSIC_MUSIC_RESOURCES") else {
            return;
        };
        let mut r = SoundRegistry::load(std::path::Path::new(&root));
        let disc = r.get("minecraft:music_disc.pigstep").cloned();
        assert!(sync(&mut r, true));
        for event in [
            "minecraft:music.game",
            "minecraft:music.creative",
            "minecraft:music.under_water",
            "minecraft:music.overworld.swamp",
            "minecraft:music.menu",
            "minecraft:music.nether.nether_wastes",
            "minecraft:music.end",
        ] {
            let pool = policy::group(event).unwrap();
            assert_eq!(r.tracks(event).len(), pool.tracks().len(), "{event}");
            let mut random = horizon_core::random::LegacyRandom::new(7);
            for _ in 0..128 {
                let sound = r
                    .sound(event, &mut random)
                    .expect("nonempty classic playlist");
                assert!(pool.allows(&sound.location), "{event}: {}", sound.location);
            }
        }
        assert_eq!(r.get("minecraft:music_disc.pigstep"), disc.as_ref());
    }
}
