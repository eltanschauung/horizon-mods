//! The mod's own files: values the settings can't hold and where each world's placements are,
//! in the mod's data folder (`<game>/horizon-mod-data/horizonmatica/`), and the schematics folder
//! (`<game>/schematics/`, as Litematica's). Versions before 0.4.0 kept all of it among the
//! settings (`horizon-mods/.horizon-mod-data/horizonmatica/<key>`): what isn't in the data
//! folder yet is read from there.

use crate::world::World;
use crate::ID;
use mod_api::ConfigPath;
use std::path::PathBuf;

/// The folder the game runs in.
fn instance_dir(w: &World<'_>) -> PathBuf {
    w.host.instance_dir()
}

pub fn schematics_dir(w: &World<'_>) -> Result<PathBuf, String> {
    let dir = instance_dir(w).join("schematics");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(dir)
}

/// What an older version left under `key`.
fn legacy(w: &World<'_>, key: &str) -> Option<String> {
    let path = instance_dir(w).join("horizon-mods").join(".horizon-mod-data").join(ID).join(key);
    std::fs::read_to_string(path).ok()
}

/// A file name the data folder takes: anything but `a-z 0-9 . _ -` becomes `_`.
fn file_name(key: &str) -> String {
    key.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') { c.to_ascii_lowercase() } else { '_' }).collect()
}

/// The value kept under `key`, or what an older version kept there.
pub fn read_value(w: &World<'_>, key: &str) -> Option<String> {
    let path = ConfigPath::new(&file_name(key)).ok()?;
    match w.host.data_files().read(&path) {
        Ok(bytes) => String::from_utf8(bytes).ok(),
        Err(_) => legacy(w, key),
    }
}

pub fn write_value(w: &World<'_>, key: &str, value: &str) {
    let Ok(path) = ConfigPath::new(&file_name(key)) else { return };
    if let Err(e) = w.host.data_files().write(&path, value.as_bytes()) {
        eprintln!("[Horizonmatica] can't keep {key}: {e:?}");
    }
}

/// The key of the world played and its dimension: a single-player world by its save folder, a
/// server by its address. None outside a world.
pub fn world_key(w: &World<'_>) -> Option<String> {
    if !w.in_world() {
        return None;
    }
    let server = w.host.client_session().client_server();
    let world = match server {
        Some(s) if !s.integrated_local => format!("server_{}", s.address.unwrap_or_default()),
        Some(s) => format!("world_{}", s.save_folder.unwrap_or_default()),
        None => "world_".to_string(),
    };
    // As before 0.4.0 (the game's dimension enum by name), so older files are found.
    let dim = match w.dimension().as_deref() {
        Some("minecraft:overworld") | None => "Overworld".to_string(),
        Some("minecraft:the_nether") => "Nether".to_string(),
        Some("minecraft:the_end") => "End".to_string(),
        Some(other) => other.to_string(),
    };
    let key = format!("placed.{world}.{dim}");
    Some(key.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') { c } else { '_' }).collect())
}
