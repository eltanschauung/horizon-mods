// SPDX-License-Identifier: EUPL-1.2-or-later
use mod_api::serde_json::{self, Value, json};
use std::{path::PathBuf, sync::OnceLock};
fn config_path() -> PathBuf {
    let root = std::env::var_os("HORIZON_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share/horizon")
        });
    root.join("config/arcadiandream.json")
}

pub fn config() -> &'static Value {
    static CONFIG: OnceLock<Value> = OnceLock::new();
    CONFIG.get_or_init(||{
        let path = config_path();
        let defaults=json!({
            "chiselOptions":{"canUse":true,"maxDistance":16},"nueTridentOptions":{"canHaveImpaling":true},
            "hisouSwordOptions":{"minHeightForPeaches":196,"canDisableShields":true},"mochiMalletOptions":{"lowViolence":false},
            "houraiElixirOptions":{"canDrink":false,"nerfElixir":true,"allowDying":false,"elixirFatigueThreshold":1},
            "foldingChairOptions":{"knockbackStrength":1.5},"icicleSwordOptions":{"meltsInstantly":true},
            "fairyCharmOptions":{"canPreventSpawning":true,"distance":128},"danmakuCooldownMultiplier":1,"danmakuDamageMultiplier":1,
            "cooldownPerBulletType":true,"fairyOptions":{"spawnFairies":true,"spawnSunflowerFairies":true,"spawnIceFairies":true}});
        let mut result=defaults.clone();if let Ok(b)=std::fs::read(&path) {match serde_json::from_slice::<Value>(&b){Ok(v)=>merge(&mut result,v),Err(e)=>eprintln!("Arcadian Dream: invalid config {e}; using defaults")}}
        else {let _=std::fs::create_dir_all(path.parent().unwrap());let _=std::fs::write(path,serde_json::to_vec_pretty(&defaults).unwrap());}result
    })
}
fn merge(a: &mut Value, b: Value) {
    if let (Some(a), Some(b)) = (a.as_object_mut(), b.as_object()) {
        for (k, v) in b {
            if let Some(old) = a.get_mut(k) {
                if old.is_object() {
                    merge(old, v.clone())
                } else {
                    *old = v.clone()
                }
            }
        }
    }
}

const SETTINGS: &[(&str, &str, &str, &str)] = &[
    (
        "chisel.enabled",
        "Wall-passing chisel",
        "chiselOptions",
        "canUse",
    ),
    (
        "chisel.distance",
        "Chisel maximum distance",
        "chiselOptions",
        "maxDistance",
    ),
    (
        "trident.impaling",
        "Nue trident accepts Impaling",
        "nueTridentOptions",
        "canHaveImpaling",
    ),
    (
        "hisou.peach_height",
        "Hisou Sword peach altitude",
        "hisouSwordOptions",
        "minHeightForPeaches",
    ),
    (
        "hisou.disable_shield",
        "Hisou Sword disables shields",
        "hisouSwordOptions",
        "canDisableShields",
    ),
    (
        "mallet.low_violence",
        "Mochi Mallet low violence",
        "mochiMalletOptions",
        "lowViolence",
    ),
    (
        "elixir.enabled",
        "Allow Hourai Elixir",
        "houraiElixirOptions",
        "canDrink",
    ),
    (
        "elixir.fatigue",
        "Hourai Elixir causes fatigue",
        "houraiElixirOptions",
        "nerfElixir",
    ),
    (
        "elixir.can_die",
        "Hourai Elixir allows death",
        "houraiElixirOptions",
        "allowDying",
    ),
    (
        "elixir.threshold",
        "Elixir fatigue threshold",
        "houraiElixirOptions",
        "elixirFatigueThreshold",
    ),
    (
        "chair.knockback",
        "Folding Chair knockback",
        "foldingChairOptions",
        "knockbackStrength",
    ),
    (
        "icicle.melt",
        "Icicle Sword melts instantly in the Nether",
        "icicleSwordOptions",
        "meltsInstantly",
    ),
    (
        "fairy_charm.prevent",
        "Fairy Charm prevents spawning",
        "fairyCharmOptions",
        "canPreventSpawning",
    ),
    (
        "fairy_charm.distance",
        "Fairy Charm distance",
        "fairyCharmOptions",
        "distance",
    ),
    (
        "danmaku.cooldown",
        "Danmaku cooldown multiplier",
        "",
        "danmakuCooldownMultiplier",
    ),
    (
        "danmaku.damage",
        "Danmaku damage multiplier",
        "",
        "danmakuDamageMultiplier",
    ),
    (
        "danmaku.family_cooldown",
        "Separate cooldown for each danmaku family",
        "",
        "cooldownPerBulletType",
    ),
    (
        "fairy.spawn",
        "Spawn fairies",
        "fairyOptions",
        "spawnFairies",
    ),
    (
        "fairy.sunflower",
        "Spawn sunflower fairies",
        "fairyOptions",
        "spawnSunflowerFairies",
    ),
    (
        "fairy.ice",
        "Spawn ice fairies",
        "fairyOptions",
        "spawnIceFairies",
    ),
];

pub fn register(registrar: &mut dyn mod_api::Registrar) {
    let current = config();
    for &(key, label, section, field) in SETTINGS {
        let value = if section.is_empty() {
            &current[field]
        } else {
            &current[section][field]
        };
        let setting = if let Some(default) = value.as_bool() {
            mod_api::SettingSpec::toggle(key, label, default)
        } else {
            let (min, max, step): (f64, f64, f64) = match key {
                "chisel.distance" => (1.0, 64.0, 1.0),
                "hisou.peach_height" => (64.0, 256.0, 8.0),
                "elixir.threshold" => (1.0, 5.0, 1.0),
                "chair.knockback" => (0.0, 10.0, 0.5),
                "fairy_charm.distance" => (25.0, 128.0, 1.0),
                _ => (1.0, 4.0, 1.0),
            };
            let default = value.as_f64().unwrap_or(min).clamp(min, max);
            mod_api::SettingSpec::range(key, label, min, max, step, default)
        };
        let _ = registrar.register_setting(setting.tooltip("Applies when the game restarts."));
    }
}

pub fn changed(host: &mut dyn mod_api::ClientHost, setting: &str) {
    let Some(&(_, _, section, field)) = SETTINGS.iter().find(|&&(key, _, _, _)| key == setting)
    else {
        return;
    };
    let Some(value) = host
        .setting_bool(setting)
        .map(|v| json!(v))
        .or_else(|| host.setting_f64(setting).map(|v| json!(v)))
    else {
        return;
    };
    let path = config_path();
    let mut data = config().clone();
    if let Ok(old) = std::fs::read(&path)
        && let Ok(old) = serde_json::from_slice(&old)
    {
        merge(&mut data, old);
    }
    if section.is_empty() {
        data[field] = value;
    } else {
        data[section][field] = value;
    }
    let save = (|| -> std::io::Result<()> {
        std::fs::create_dir_all(path.parent().unwrap())?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(&data)?)?;
        std::fs::rename(tmp, path)
    })();
    if let Err(error) = save {
        eprintln!("Arcadian Dream settings: {error}");
    }
}
