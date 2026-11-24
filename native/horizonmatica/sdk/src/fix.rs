//! Old schematics brought up to date: block states saved by an older game (1.12's numeric-era
//! names, renamed blocks, changed properties) go through the game's own data fixer, given by the
//! game side (`src/native/game_support.rs`) as `Fixer`.

use horizonmatica_core::{BlockState, Schematic};
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// One block state (name, properties) from data version `from` to the game's.
pub type Fixer = fn(name: &str, properties: &[(String, String)], from: i32) -> Option<(String, Vec<(String, String)>)>;

/// The fixer and the game's data version.
static FIXER: OnceLock<(Fixer, i32)> = OnceLock::new();

/// From the game side, once.
pub fn install(
    fixer: Fixer,
    data_version: i32,
    legacy_path: impl FnOnce() -> std::path::PathBuf,
    read_legacy: fn(&std::path::Path) -> Result<Vec<String>, String>,
) {
    FIXER.get_or_init(|| {
        horizonmatica_core::formats::set_legacy_source(legacy_path(), read_legacy);
        (fixer, data_version)
    });
}

/// Brings the palettes of a schematic saved by an older game up to date. How many states changed.
pub fn upgrade(schematic: &mut Schematic) -> usize {
    let Some(&(fixer, to)) = FIXER.get() else { return 0 };
    let from = i32::try_from(schematic.data_version).unwrap_or(0);
    if from <= 0 || from >= to {
        return 0;
    }
    let mut changed = 0;
    for region in &mut schematic.regions {
        for state in &mut region.palette {
            if state.is_air() {
                continue;
            }
            let props: Vec<(String, String)> = state.properties.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            let Some((name, properties)) = fixer(&state.name, &props, from) else { continue };
            let new = BlockState { name, properties: properties.into_iter().collect::<BTreeMap<_, _>>() };
            if new != *state {
                *state = new;
                changed += 1;
            }
        }
    }
    schematic.data_version = i64::from(to);
    changed
}
