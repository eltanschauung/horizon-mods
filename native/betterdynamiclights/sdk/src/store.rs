// The settings as the game keeps them. The mod's page in Options > Mods has two rows, as
// before: "Settings: Open..." (opens the settings screen) and the main switch, both registered
// settings the game draws and keeps. The other settings and the toggle key are the screen's,
// one file per setting in the mod's data folder (`horizon-mod-data/dynamiclights/`).
//
// A change is in effect at once and written at the next tick: the screen and frame hooks only
// read the game.
use crate::runtime::{self as dl, Settings};
use mod_api::*;
use std::sync::Mutex;

/// The page's row that opens the settings screen: a choice whose two values both read
/// "Open...", so every press is a change.
pub(crate) const CONFIGURE: &str = "config_screen";
/// The page's main switch.
const ENABLED: &str = "enabled";

pub(crate) fn register(r: &mut dyn Registrar) {
    let mut configure = SettingSpec::choice(
        CONFIGURE,
        "dynamiclights.setting.configure",
        ["dynamiclights.setting.open", "dynamiclights.setting.open_again"],
        0,
    );
    configure.tooltip = Some("dynamiclights.setting.configure.tooltip".into());
    let _ = r.register_setting(configure);
    let mut enabled = SettingSpec::toggle(ENABLED, "dynamiclights.setting.enabled", true);
    enabled.tooltip = Some("dynamiclights.setting.enabled.tooltip".into());
    let _ = r.register_setting(enabled);
}

/// What the files hold, and the toggle key in effect and in its file.
#[derive(Default)]
struct Kept {
    written: Option<Settings>,
    key: Option<String>,
    key_written: Option<String>,
}

static KEPT: Mutex<Kept> = Mutex::new(Kept { written: None, key: None, key_written: None });

fn kept<R>(f: impl FnOnce(&mut Kept) -> R) -> R {
    f(&mut KEPT.lock().unwrap_or_else(|e| e.into_inner()))
}

fn file(key: &str) -> Option<ConfigPath> {
    ConfigPath::new(key).ok()
}

fn read(host: &dyn Host, key: &str) -> Option<String> {
    if key == ENABLED {
        return host.setting(dl::ID, ENABLED);
    }
    let bytes = host.data_files().read(&file(key)?).ok()?;
    String::from_utf8(bytes).ok()
}

/// The settings in effect (read once).
pub(crate) fn settings(host: &dyn Host) -> Settings {
    let s = dl::settings(|key| read(host, key));
    kept(|k| {
        k.written.get_or_insert(s);
    });
    s
}

/// New settings, in effect at once, written at the next tick.
pub(crate) fn set(s: Settings) {
    dl::set_settings(s);
}

/// The key that turns the lights on and off (unbound at first).
pub(crate) fn toggle_key(host: &dyn Host) -> String {
    if let Some(key) = kept(|k| k.key.clone()) {
        return key;
    }
    let key = read(host, dl::TOGGLE_KEY)
        .map(|k| k.trim().to_owned())
        .filter(|k| !k.is_empty())
        .unwrap_or_else(|| dl::UNBOUND.to_owned());
    kept(|k| {
        k.key = Some(key.clone());
        k.key_written = Some(key.clone());
    });
    key
}

/// A new toggle key, written at the next tick.
pub(crate) fn set_toggle_key(key: &str) {
    kept(|k| k.key = Some(key.to_owned()));
}

/// Each tick: what changed since the files were written.
pub(crate) fn flush(host: &dyn Host) {
    let s = settings(host);
    let old = kept(|k| k.written.replace(s));
    if old != Some(s) {
        let before = old.map(|o| o.stored()).unwrap_or_default();
        for (key, value) in s.stored() {
            if before.iter().any(|(k, v)| *k == key && *v == value) {
                continue;
            }
            if key == ENABLED {
                host.set_setting(dl::ID, ENABLED, &value);
            } else if let Some(path) = file(key) {
                let _ = host.data_files().write(&path, value.as_bytes());
            }
        }
    }
    let key = kept(|k| (k.key != k.key_written).then(|| k.key.clone()).flatten());
    if let Some(key) = key
        && let Some(path) = file(dl::TOGGLE_KEY)
        && host.data_files().write(&path, key.as_bytes()).is_ok()
    {
        kept(|k| k.key_written = Some(key));
    }
}

/// The main switch changed on the mod's page (the game wrote it).
pub(crate) fn setting_changed(host: &dyn Host, key: &str) {
    if key == ENABLED {
        let mut s = settings(host);
        s.enabled = host.setting(dl::ID, ENABLED).as_deref().map(str::trim) != Some("false");
        dl::set_settings(s);
        kept(|k| {
            if let Some(w) = &mut k.written {
                w.enabled = s.enabled;
            }
        });
    }
}
