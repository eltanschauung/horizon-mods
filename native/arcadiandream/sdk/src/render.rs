// SPDX-License-Identifier: EUPL-1.2-or-later
// Geometry and keyframes originate in Arcadian Dream, CC-BY-SA-4.0.
use mod_api::serde_json::{self, Value};
use mod_api::{BlockEntityDrawContext, ClientHost, CubeFace, EquipmentSlot, LightLayer, MobRender};
use std::sync::LazyLock;

static MODELS: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../assets/arcadiandream/embedded/models.json")).expect("embedded fairy geometry")
});

fn vector(v: &Value, default: [f32; 3]) -> [f32; 3] {
    if let Some(a) = v.as_array() {
        std::array::from_fn(|i| {
            a.get(i)
                .and_then(Value::as_f64)
                .unwrap_or(default[i] as f64) as f32
        })
    } else {
        default
    }
}
fn frame(v: &Value, time: f32, default: [f32; 3]) -> [f32; 3] {
    if v.is_array() {
        return vector(v, default);
    }
    if v["vector"].is_array() {
        return vector(&v["vector"], default);
    }
    let Some(keys) = v.as_object() else {
        return default;
    };
    let mut frames = keys
        .iter()
        .filter_map(|(s, v)| s.parse::<f32>().ok().map(|t| (t, v)))
        .collect::<Vec<_>>();
    frames.sort_by(|a, b| a.0.total_cmp(&b.0));
    if frames.is_empty() {
        return default;
    }
    let before = frames
        .iter()
        .rev()
        .find(|(t, _)| *t <= time)
        .unwrap_or(&frames[0]);
    let after = frames
        .iter()
        .find(|(t, _)| *t >= time)
        .unwrap_or(frames.last().unwrap());
    let a = vector(
        before
            .1
            .get("post")
            .or_else(|| before.1.get("vector"))
            .unwrap_or(before.1),
        default,
    );
    let b = vector(
        after
            .1
            .get("pre")
            .or_else(|| after.1.get("vector"))
            .unwrap_or(after.1),
        default,
    );
    let mut t = if after.0 == before.0 {
        0.0
    } else {
        (time - before.0) / (after.0 - before.0)
    };
    match after.1["easing"].as_str().unwrap_or("linear") {
        "easeInOutQuad" => {
            t = if t < 0.5 {
                2.0 * t * t
            } else {
                1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
            }
        }
        "easeInCirc" => t = 1.0 - (1.0 - t * t).max(0.0).sqrt(),
        "easeInOutCirc" => {
            t = if t < 0.5 {
                (1.0 - (1.0 - (2.0 * t).powi(2)).max(0.0).sqrt()) / 2.0
            } else {
                ((1.0 - (-2.0 * t + 2.0).powi(2)).max(0.0).sqrt() + 1.0) / 2.0
            }
        }
        "easeInOutSine" => t = (1.0 - (std::f32::consts::PI * t).cos()) / 2.0,
        "easeOutElastic" => {
            // GeckoLib 4's parameterized elastic curve, not easings.net's fixed-period curve.
            let n = after.1["easingArgs"][0].as_f64().unwrap_or(1.0) as f32;
            let u = 1.0 - t;
            t = (u * std::f32::consts::FRAC_PI_2).cos().powi(3)
                * (u * n * std::f32::consts::PI).cos();
        }
        _ => {}
    }
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}
// GeckoLib's controller keeps an independent clip clock and pose snapshot per entity.
#[derive(Clone, Copy, Debug)]
struct BonePose {
    position: [f32; 3],
    rotation: [f32; 3],
    scale: [f32; 3],
}
impl BonePose {
    fn sample(anim: &Value, name: &str, time: f32) -> Self {
        let b = &anim["bones"][name];
        Self {
            position: frame(&b["position"], time, [0.0; 3]),
            rotation: frame(&b["rotation"], time, [0.0; 3]),
            scale: frame(&b["scale"], time, [1.0; 3]),
        }
    }
    fn blend(self, to: Self, t: f32) -> Self {
        let lerp = |a: [f32; 3], b: [f32; 3]| std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t);
        Self {
            position: lerp(self.position, to.position),
            rotation: lerp(self.rotation, to.rotation),
            scale: lerp(self.scale, to.scale),
        }
    }
}
#[derive(Default)]
struct Controller {
    clip: &'static str,
    changed: f32,
    last: f32,
    from: std::collections::HashMap<String, BonePose>,
    shown: std::collections::HashMap<String, BonePose>,
}
fn select_clip(moving: bool, hurt: bool, ground: bool) -> &'static str {
    if moving && !hurt {
        "move"
    } else if ground {
        "idle.ground"
    } else {
        "idle"
    }
}
impl Controller {
    fn sample(
        &mut self,
        clip: &'static str,
        age: f32,
        animations: &Value,
        bones: &[Value],
    ) -> std::collections::HashMap<String, BonePose> {
        if age < self.last {
            *self = Self::default();
        }
        if self.clip != clip {
            self.from = self.shown.clone();
            self.clip = clip;
            self.changed = age;
        }
        self.last = age;
        let elapsed = (age - self.changed).max(0.0);
        let anim = &animations[clip];
        let length = anim["animation_length"].as_f64().unwrap_or(4.0) as f32;
        // Java transitions toward the first keyframe for 5 ticks, then starts the loop.
        let time = ((elapsed - 5.0).max(0.0) / 20.0).rem_euclid(length);
        self.shown = bones
            .iter()
            .map(|b| {
                let name = b["name"].as_str().unwrap_or("");
                let target = BonePose::sample(anim, name, time);
                let default = BonePose {
                    position: [0.0; 3],
                    rotation: [0.0; 3],
                    scale: [1.0; 3],
                };
                let pose = self
                    .from
                    .get(name)
                    .copied()
                    .unwrap_or(default)
                    .blend(target, (elapsed / 5.0).min(1.0));
                (name.to_owned(), pose)
            })
            .collect();
        self.shown.clone()
    }
}
thread_local! { static CONTROLLERS: std::cell::RefCell<std::collections::HashMap<u32,Controller>> = std::cell::RefCell::new(std::collections::HashMap::new()); }
// Model JSON -> GeckoLib baked coordinates. Apply consistently to cubes and pivots.
fn geo_position(v: [f32; 3]) -> [f32; 3] {
    [-v[0], v[1], v[2]]
}
fn geo_rotation(v: [f32; 3]) -> [f32; 3] {
    [-v[0], -v[1], v[2]]
}
fn death_roll(ticks: i32, partial: f32) -> f32 {
    if ticks <= 0 {
        return 0.0;
    }
    (((ticks as f32 + partial - 1.0) / 20.0 * 1.6)
        .max(0.0)
        .sqrt())
    .min(1.0)
        * 90.0
}
pub fn draw_fairy(
    host: &dyn ClientHost,
    m: &MobRender<'_>,
    draw: &mut dyn BlockEntityDrawContext,
) -> bool {
    let Some(name) = m.kind.strip_prefix("arcadiandream:") else {
        return false;
    };
    if !["fairy", "sunflower_fairy", "ice_fairy"].contains(&name) {
        return false;
    }
    let model = &MODELS[format!("geo/entity/{name}.geo.json")]["minecraft:geometry"][0];
    let Some(bones) = model["bones"].as_array() else {
        return false;
    };
    let animname = if name == "sunflower_fairy" {
        name
    } else {
        "fairy"
    };
    let animations = &MODELS[format!("animations/entity/{animname}.animation.json")]["animations"];
    let mob = host.entity(m.id);
    let moving = mob
        .and_then(|mob| mob.velocity(host))
        .is_some_and(|v| (v[0].abs() + v[2].abs()) / 2.0 >= 0.015 && m.walk_speed != 0.0);
    let clip = select_clip(moving, m.hurt_time > 0, m.on_ground);
    let poses = CONTROLLERS.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() > 256 {
            cache.retain(|id, _| host.entity(*id).is_some());
        }
        cache
            .entry(m.id)
            .or_default()
            .sample(clip, m.age, animations, bones)
    });
    let skin = if name == "fairy" {
        format!(
            "fairy_{}",
            ["blue", "red", "green", "yellow"][crate::fairies::variant(m.id)]
        )
    } else {
        name.to_string()
    };
    let sprite = format!("arcadiandream:entity/fairy/{skin}");
    let wing_sprite = format!("arcadiandream:entity/fairy/{name}_wings");
    let tw = model["description"]["texture_width"]
        .as_f64()
        .unwrap_or(64.0) as f32;
    let th = model["description"]["texture_height"]
        .as_f64()
        .unwrap_or(64.0) as f32;

    // The Allay template supplies full block light before native rendering.
    // Fairies use ordinary world lighting, except while actually burning.
    let inherited_light = draw.light();
    if let Some(mob) = mob
        && let Some(level) = mob.level(host)
        && let Some(eye) = mob.eye_position(host)
    {
        let pos = eye.map(|v| v.floor() as i32);
        let block = if mob.fire_ticks(host).unwrap_or(0) > 0 {
            15
        } else {
            level
                .brightness(host, LightLayer::Block, pos)
                .unwrap_or(0)
                .clamp(0, 15) as u32
        };
        let sky = level
            .brightness(host, LightLayer::Sky, pos)
            .unwrap_or(0)
            .clamp(0, 15) as u32;
        draw.set_light(sky * 16 | block * 16 << 8);
    }
    draw.push();
    draw.rotate_about([0.0, 1.0, 0.0], 180.0, [0.0; 3]);
    if m.death_time > 0 {
        draw.rotate_about(
            [0.0, 0.0, 1.0],
            death_roll(m.death_time, m.partial),
            [0.0; 3],
        );
    }
    for (i, b) in bones.iter().enumerate() {
        if b["parent"].is_null() {
            bone(
                draw,
                bones,
                i,
                &poses,
                0.0,
                m,
                &sprite,
                &wing_sprite,
                tw,
                th,
            );
        }
    }
    draw.pop();
    draw.set_light(inherited_light);
    true
}

fn bone(
    draw: &mut dyn BlockEntityDrawContext,
    bones: &[Value],
    index: usize,
    poses: &std::collections::HashMap<String, BonePose>,
    _time: f32,
    m: &MobRender<'_>,
    sprite: &str,
    wings: &str,
    tw: f32,
    th: f32,
) {
    let b = &bones[index];
    let name = b["name"].as_str().unwrap_or("");
    let pose = poses.get(name).copied().unwrap_or(BonePose {
        position: [0.0; 3],
        rotation: [0.0; 3],
        scale: [1.0; 3],
    });
    draw.push();
    draw.translate(geo_position(pose.position).map(|v| v / 16.0));
    let pivot = geo_position(vector(&b["pivot"], [0.0; 3])).map(|v| v / 16.0);
    let base = geo_rotation(vector(&b["rotation"], [0.0; 3]));
    let delta = geo_rotation(pose.rotation);
    let mut rotation = std::array::from_fn::<_, 3, _>(|i| base[i] + delta[i]);
    if name == "head" || name == "bipedHead" {
        // Java's custom head animation replaces X/Y, rather than adding to the clip.
        rotation[0] = m.pitch;
        rotation[1] = m.head_yaw;
    }
    draw.translate(pivot);
    for i in [2, 1, 0] {
        let mut axis = [0.0; 3];
        axis[i] = 1.0;
        draw.rotate_about(axis, rotation[i], [0.0; 3]);
    }
    draw.scale(pose.scale);
    draw.translate(pivot.map(|v| -v));
    if let Some(cubes) = b["cubes"].as_array() {
        for c in cubes {
            draw.push();
            let mut origin = vector(&c["origin"], [0.0; 3]);
            let size = vector(&c["size"], [0.0; 3]);
            origin[0] = -origin[0] - size[0];
            let inflate = c["inflate"].as_f64().unwrap_or(0.0) as f32;
            let from = std::array::from_fn(|i| origin[i] - inflate);
            let to = std::array::from_fn(|i| origin[i] + size[i] + inflate);
            let cp = geo_position(vector(&c["pivot"], [0.0; 3])).map(|v| v / 16.0);
            let r = geo_rotation(vector(&c["rotation"], [0.0; 3]));
            for i in [2, 1, 0] {
                let mut axis = [0.0; 3];
                axis[i] = 1.0;
                draw.rotate_about(axis, r[i], cp);
            }
            let s = if name.to_ascii_lowercase().contains("wing") {
                wings
            } else {
                sprite
            };
            let faces = std::array::from_fn(|i| {
                let face = ["down", "up", "north", "south", "west", "east"][i];
                let v = &c["uv"][face];
                let uv = if v.is_object() {
                    let u = vector(&v["uv"], [0.0; 3]);
                    let n = vector(&v["uv_size"], [0.0; 3]);
                    [
                        u[0] * 16.0 / tw,
                        u[1] * 16.0 / th,
                        (u[0] + n[0]) * 16.0 / tw,
                        (u[1] + n[1]) * 16.0 / th,
                    ]
                } else if let Some(a) = c["uv"].as_array() {
                    let u = a[0].as_f64().unwrap_or(0.0) as f32;
                    let v = a[1].as_f64().unwrap_or(0.0) as f32;
                    let [x, y, z] = size;
                    let (du, dv, w, h) = match face {
                        "down" => (z + x, 0.0, x, z),
                        "up" => (z, 0.0, x, z),
                        "north" => (z, z, x, y),
                        "south" => (z + x + z, z, x, y),
                        "west" => (z + x, z, z, y),
                        _ => (0.0, z, z, y),
                    };
                    [
                        (u + du) * 16.0 / tw,
                        (v + dv) * 16.0 / th,
                        (u + du + w) * 16.0 / tw,
                        (v + dv + h) * 16.0 / th,
                    ]
                } else {
                    return None;
                };
                Some(CubeFace::new(s).with_uv(uv))
            });
            draw.cube(
                from,
                to,
                faces,
                if m.hurt_time > 0 || m.death_time > 0 {
                    0xff8888
                } else {
                    0xffffff
                },
            );
            draw.pop();
        }
    }
    for (j, child) in bones.iter().enumerate() {
        if child["parent"].as_str() == Some(name) {
            bone(draw, bones, j, poses, 0.0, m, sprite, wings, tw, th);
        }
    }
    draw.pop();
}

pub fn draw_hat(host: &dyn ClientHost, m: &MobRender<'_>, draw: &mut dyn BlockEntityDrawContext) {
    let wearing = host
        .entity(m.id)
        .and_then(|entity| host.living(entity))
        .and_then(|living| living.equipment(host, EquipmentSlot::Head))
        .is_some_and(|stack| stack.item().id(host) == "arcadiandream:ordinary_hat");
    if !wearing {
        return;
    }
    let model = &MODELS["geo/armor/ordinary_armor.geo.json"]["minecraft:geometry"][0];
    let Some(bones) = model["bones"].as_array() else {
        return;
    };
    draw.push();
    draw.rotate_about([0.0, 1.0, 0.0], 180.0, [0.0; 3]);
    for (i, b) in bones.iter().enumerate() {
        if b["parent"].is_null() {
            bone(
                draw,
                bones,
                i,
                &std::collections::HashMap::new(),
                m.age / 20.0,
                m,
                "arcadiandream:models/armor/ordinary_armor",
                "arcadiandream:models/armor/ordinary_armor",
                64.0,
                64.0,
            );
        }
    }
    draw.pop();
}
