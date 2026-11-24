//! Babies drawn as they were before the baby redesign: each one the grown mob's model and textures
//! (and its overlays: wool, collars, a villager's clothes, a horse's markings), at the old sizes.
//!
//! Most were `AgeableListModel`s: the head drawn on its own, kept big (or a little smaller), moved
//! down and forward, and the body at half size, moved down to the ground. Villagers and squid were the
//! grown model at half size. The numbers are each model's from before the redesign.
//!
//! The server side reads what picks a baby's textures (its variant, colours, villager type) from its
//! saved data, a few times a second, for the drawing to use: a mob's drawing can't read it.

use std::collections::{HashMap, HashSet};
use std::f32::consts::PI;
use std::sync::{LazyLock, Mutex};

use mod_api::{Compound, Flow, RenderLivingPreEvent, ModelPart, Tag};

/// How a baby is shrunk.
#[derive(Clone, Copy)]
enum Shape {
    /// `AgeableListModel`: the head at `head_scale`, moved by `head` (y, z in pixels); the rest at
    /// `body_scale`, moved down by `body_y` pixels.
    Split { head_scale: f32, head: [f32; 2], body_scale: f32, body_y: f32 },
    /// The whole grown model at this size.
    Whole(f32),
}

/// How its parts move.
#[derive(Clone, Copy, PartialEq)]
enum Anim {
    Quadruped,
    Chicken,
    /// Zombies' arms held out.
    Zombie,
    /// Arms swinging as it walks.
    Humanoid,
    Villager,
    Horse,
    Still,
}

struct Old {
    kind: &'static str,
    layer: &'static str,
    /// The texture when the server hasn't said which yet.
    texture: &'static str,
    shape: Shape,
    anim: Anim,
    /// The model's root parts drawn as the head.
    head: &'static [&'static str],
}

const QUAD_HEAD: &[&str] = &["head"];
const HUMAN_HEAD: &[&str] = &["head", "hat"];
const fn quad(y: f32, z: f32) -> Shape {
    Shape::Split { head_scale: 1.0, head: [y, z], body_scale: 0.5, body_y: 24.0 }
}
/// `HumanoidModel`'s: `AgeableListModel(true, 16, 0, 2, 2, 24)`.
const HUMANOID: Shape = Shape::Split { head_scale: 0.75, head: [16.0, 0.0], body_scale: 0.5, body_y: 24.0 };
/// `HoglinModel`'s: `(true, 8, 6, 1.9, 2, 24)`.
const HOGLIN: Shape = Shape::Split { head_scale: 1.5 / 1.9, head: [8.0, 6.0], body_scale: 0.5, body_y: 24.0 };
/// `HorseModel`'s: `(true, 16.2, 1.36, 2.7272, 2, 20)`.
const HORSE: Shape = Shape::Split { head_scale: 1.5 / 2.7272, head: [16.2, 1.36], body_scale: 0.5, body_y: 20.0 };

const OLD: &[Old] = &[
    Old { kind: "minecraft:cow", layer: "minecraft:cow#main", texture: "entity/cow/cow_temperate", shape: quad(10.0, 4.0), anim: Anim::Quadruped, head: QUAD_HEAD },
    Old { kind: "minecraft:mooshroom", layer: "minecraft:mooshroom#main", texture: "entity/cow/mooshroom_red", shape: quad(10.0, 4.0), anim: Anim::Quadruped, head: QUAD_HEAD },
    Old { kind: "minecraft:pig", layer: "minecraft:pig#main", texture: "entity/pig/pig_temperate", shape: quad(4.0, 4.0), anim: Anim::Quadruped, head: QUAD_HEAD },
    Old { kind: "minecraft:sheep", layer: "minecraft:sheep#main", texture: "entity/sheep/sheep", shape: quad(8.0, 4.0), anim: Anim::Quadruped, head: QUAD_HEAD },
    Old { kind: "minecraft:chicken", layer: "minecraft:chicken#main", texture: "entity/chicken/chicken_temperate", shape: quad(5.0, 2.0), anim: Anim::Chicken, head: QUAD_HEAD },
    Old { kind: "minecraft:wolf", layer: "minecraft:wolf#main", texture: "entity/wolf/wolf", shape: quad(5.0, 2.0), anim: Anim::Quadruped, head: QUAD_HEAD },
    // `OcelotModel`: `(true, 10, 4)`.
    Old {
        kind: "minecraft:cat",
        layer: "minecraft:cat#main",
        texture: "entity/cat/cat_tabby",
        shape: Shape::Split { head_scale: 0.75, head: [10.0, 4.0], body_scale: 0.5, body_y: 24.0 },
        anim: Anim::Quadruped,
        head: QUAD_HEAD,
    },
    Old {
        kind: "minecraft:ocelot",
        layer: "minecraft:ocelot#main",
        texture: "entity/cat/ocelot",
        shape: Shape::Split { head_scale: 0.75, head: [10.0, 4.0], body_scale: 0.5, body_y: 24.0 },
        anim: Anim::Quadruped,
        head: QUAD_HEAD,
    },
    // `RabbitModel.renderToBuffer`: the head at 0.5667, the body at 0.4.
    Old {
        kind: "minecraft:rabbit",
        layer: "minecraft:rabbit#main",
        texture: "entity/rabbit/rabbit_brown",
        shape: Shape::Split { head_scale: 0.566_666_7, head: [22.0, 2.0], body_scale: 0.4, body_y: 36.0 },
        anim: Anim::Still,
        head: &["head", "left_ear", "right_ear", "nose"],
    },
    Old { kind: "minecraft:zombie", layer: "minecraft:zombie#main", texture: "entity/zombie/zombie", shape: HUMANOID, anim: Anim::Zombie, head: HUMAN_HEAD },
    Old { kind: "minecraft:husk", layer: "minecraft:husk#main", texture: "entity/zombie/husk", shape: HUMANOID, anim: Anim::Zombie, head: HUMAN_HEAD },
    Old { kind: "minecraft:drowned", layer: "minecraft:drowned#main", texture: "entity/zombie/drowned", shape: HUMANOID, anim: Anim::Zombie, head: HUMAN_HEAD },
    Old {
        kind: "minecraft:zombie_villager",
        layer: "minecraft:zombie_villager#main",
        texture: "entity/zombie_villager/zombie_villager",
        shape: HUMANOID,
        anim: Anim::Zombie,
        head: HUMAN_HEAD,
    },
    Old { kind: "minecraft:piglin", layer: "minecraft:piglin#main", texture: "entity/piglin/piglin", shape: HUMANOID, anim: Anim::Humanoid, head: HUMAN_HEAD },
    Old {
        kind: "minecraft:zombified_piglin",
        layer: "minecraft:zombified_piglin#main",
        texture: "entity/piglin/zombified_piglin",
        shape: HUMANOID,
        anim: Anim::Zombie,
        head: HUMAN_HEAD,
    },
    // `VillagerRenderer.scale`: 0.9375, and half that for a child.
    Old { kind: "minecraft:villager", layer: "minecraft:villager#main", texture: "entity/villager/villager", shape: Shape::Whole(0.468_75), anim: Anim::Villager, head: &[] },
    Old { kind: "minecraft:hoglin", layer: "minecraft:hoglin#main", texture: "entity/hoglin/hoglin", shape: HOGLIN, anim: Anim::Quadruped, head: QUAD_HEAD },
    Old { kind: "minecraft:zoglin", layer: "minecraft:zoglin#main", texture: "entity/hoglin/zoglin", shape: HOGLIN, anim: Anim::Quadruped, head: QUAD_HEAD },
    Old { kind: "minecraft:horse", layer: "minecraft:horse#main", texture: "entity/horse/horse_white", shape: HORSE, anim: Anim::Horse, head: &["head_parts"] },
    Old { kind: "minecraft:donkey", layer: "minecraft:donkey#main", texture: "entity/horse/donkey", shape: HORSE, anim: Anim::Horse, head: &["head_parts"] },
    Old { kind: "minecraft:mule", layer: "minecraft:mule#main", texture: "entity/horse/mule", shape: HORSE, anim: Anim::Horse, head: &["head_parts"] },
    Old {
        kind: "minecraft:skeleton_horse",
        layer: "minecraft:skeleton_horse#main",
        texture: "entity/horse/horse_skeleton",
        shape: HORSE,
        anim: Anim::Horse,
        head: &["head_parts"],
    },
    Old {
        kind: "minecraft:zombie_horse",
        layer: "minecraft:zombie_horse#main",
        texture: "entity/horse/horse_zombie",
        shape: HORSE,
        anim: Anim::Horse,
        head: &["head_parts"],
    },
    Old { kind: "minecraft:squid", layer: "minecraft:squid#main", texture: "entity/squid/squid", shape: Shape::Whole(0.5), anim: Anim::Still, head: &[] },
    Old { kind: "minecraft:glow_squid", layer: "minecraft:glow_squid#main", texture: "entity/squid/glow_squid", shape: Shape::Whole(0.5), anim: Anim::Still, head: &[] },
];

fn old(kind: &str) -> Option<&'static Old> {
    OLD.iter().find(|o| o.kind == kind)
}

/// What a baby looks like, as its saved data says: its textures on its model, and its overlays.
#[derive(Clone, Debug, Default, PartialEq)]
struct Look {
    texture: String,
    /// Another layer (or the same) drawn over it: layer, texture, tint.
    overlays: Vec<(String, String, u32)>,
}

static LOOKS: LazyLock<Mutex<HashMap<u32, Look>>> = LazyLock::new(Default::default);

/// The looks go from the server to its players' clients on this channel: a client can't read a
/// mob's saved data, and on a server (not single player) it has nothing else to draw them from.
pub const CHANNEL: &str = "misogynist_unexpansion_native:baby_looks";
/// A message stays under the 30 000 bytes a mod message may be.
const MESSAGE_MAX: usize = 28_000;
/// Scans (of a few a second) between sending the same looks again, for players who just came.
const RESEND_EVERY: u32 = 10;

/// What was last sent for each dimension, and how many scans ago.
static SENT: LazyLock<Mutex<HashMap<String, (Vec<Vec<u8>>, u32)>>> = LazyLock::new(Default::default);

/// The looks as messages, each `first` byte (1 for the first, which replaces what the client
/// has, 0 for the rest) then one line a baby: `id\ttexture\tlayer,texture,tint;...`.
fn encode(looks: &[(u32, Look)]) -> Vec<Vec<u8>> {
    let mut messages = vec![vec![1u8]];
    for (id, look) in looks {
        let overlays: Vec<String> = look.overlays.iter().map(|(l, t, c)| format!("{l},{t},{c}")).collect();
        let line = format!("{id}\t{}\t{}\n", look.texture, overlays.join(";"));
        if messages.last().is_some_and(|m| m.len() + line.len() > MESSAGE_MAX) {
            messages.push(vec![0u8]);
        }
        if let Some(m) = messages.last_mut() {
            m.extend_from_slice(line.as_bytes());
        }
    }
    messages
}

/// A message back into looks, and whether it replaces what the client has.
fn decode(data: &[u8]) -> Option<(bool, Vec<(u32, Look)>)> {
    let (&first, rest) = data.split_first()?;
    let text = std::str::from_utf8(rest).ok()?;
    let looks = text
        .lines()
        .filter_map(|line| {
            let mut f = line.split('\t');
            let id = f.next()?.parse().ok()?;
            let texture = f.next()?.to_owned();
            let overlays = f
                .next()
                .unwrap_or("")
                .split(';')
                .filter(|o| !o.is_empty())
                .filter_map(|o| {
                    let mut p = o.split(',');
                    Some((p.next()?.to_owned(), p.next()?.to_owned(), p.next()?.parse().ok()?))
                })
                .collect();
            Some((id, Look { texture, overlays }))
        })
        .collect();
    Some((first == 1, looks))
}

/// On a client: the looks its server sent.
pub fn received(data: &[u8]) {
    let Some((replace, looks)) = decode(data) else { return };
    if let Ok(mut l) = LOOKS.lock() {
        if replace {
            l.clear();
        }
        l.extend(looks);
    }
}

pub fn clear() {
    if let Ok(mut looks) = LOOKS.lock() {
        looks.clear();
    }
    if let Ok(mut baked) = BAKED.lock() {
        baked.clear();
    }
}

/// The looks of `dimension` sent to the players in it, when they changed or now and then.
fn send(host: &mut dyn mod_api::ServerHost, level: mod_api::LevelRef, found: &[(u32, Look)]) {
    let dimension = level.dimension_id().to_owned();
    let mut sorted = found.to_vec();
    sorted.sort_by_key(|(id, _)| *id);
    let messages = encode(&sorted);
    let due = SENT.lock().is_ok_and(|mut s| {
        let entry = s.entry(dimension.to_owned()).or_default();
        entry.1 += 1;
        let due = entry.0 != messages || entry.1 >= RESEND_EVERY;
        if due {
            *entry = (messages.clone(), 0);
        }
        due
    });
    if !due {
        return;
    }
    for player in level.online_players(host).unwrap_or_default() {
        for message in &messages {
            // A play custom-payload body starts with the VarInt identifier length.
            let mut body = Vec::with_capacity(1 + CHANNEL.len() + message.len());
            body.push(CHANNEL.len() as u8);
            body.extend(CHANNEL.as_bytes());
            body.extend(message);
            let id = mod_api::Identifier::parse("minecraft:custom_payload").unwrap();
            let _ = host.player_send_packet(player, mod_api::ClientboundPlayPacket::new(id, body));
        }
    }
}

/// `DyeColor.getTextureDiffuseColor`, by dye id.
const DYES: [u32; 16] = [
    0xF9FFFE, 0xF9801D, 0xC74EBD, 0x3AB3DA, 0xFED83D, 0x80C71F, 0xF38BAA, 0x474F52, 0x9D9D97, 0x169C9C, 0x8932B8,
    0x3C44AA, 0x835432, 0x5E7C16, 0xB02E26, 0x1D1D21,
];
const HORSE_COLOURS: [&str; 7] = ["white", "creamy", "chestnut", "brown", "black", "gray", "darkbrown"];
const MARKINGS: [&str; 5] = ["", "white", "whitefield", "whitedots", "blackdots"];

fn number(tag: &Compound, key: &str) -> Option<i64> {
    match tag.get(&key.into())? {
        Tag::Byte(n) => Some(i64::from(*n)),
        Tag::Short(n) => Some(i64::from(*n)),
        Tag::Int(n) => Some(i64::from(*n)),
        Tag::Long(n) => Some(*n),
        _ => None,
    }
}

fn text(tag: &Compound, key: &str) -> Option<String> {
    match tag.get(&key.into())? {
        Tag::String(s) => {
            let s = String::from_utf16_lossy(&s.0);
            Some(s.strip_prefix("minecraft:").unwrap_or(&s).to_owned())
        }
        _ => None,
    }
}

fn is_baby(tag: &Compound) -> bool {
    number(tag, "Age").is_some_and(|a| a < 0) || number(tag, "IsBaby").is_some_and(|b| b != 0)
}

fn dye(n: Option<i64>, default: usize) -> u32 {
    DYES[n.map_or(default, |n| (n as usize) & 15)]
}

/// A villager's (or zombie villager's) type: `VillagerData.type`.
fn villager_type(tag: &Compound) -> String {
    match tag.get(&"VillagerData".into()) {
        Some(Tag::Compound(d)) => text(d, "type").unwrap_or_else(|| "plains".into()),
        _ => "plains".into(),
    }
}

fn look_of(o: &Old, tag: &Compound) -> Look {
    let mut look = Look { texture: o.texture.to_owned(), overlays: Vec::new() };
    let tame = tag.get(&"Owner".into()).is_some();
    match o.kind {
        "minecraft:mooshroom" => {
            if text(tag, "Type").as_deref() == Some("brown") {
                look.texture = "entity/cow/mooshroom_brown".into();
            }
        }
        "minecraft:sheep" => {
            if number(tag, "Sheared").unwrap_or(0) == 0 {
                let colour = dye(number(tag, "Color"), 0);
                look.overlays.push(("minecraft:sheep#wool".into(), "entity/sheep/sheep_wool".into(), colour));
            }
        }
        "minecraft:wolf" => {
            // Every wolf is the original (pale) one, whatever variant it was given.
            let file = "wolf";
            let state = if tame {
                "_tame"
            } else if tag.get(&"angry_at".into()).is_some() {
                "_angry"
            } else {
                ""
            };
            look.texture = format!("entity/wolf/{file}{state}");
            if tame {
                look.overlays.push(("minecraft:wolf#main".into(), "entity/wolf/wolf_collar".into(), dye(number(tag, "CollarColor"), 14)));
            }
        }
        "minecraft:cat" => {
            let variant = text(tag, "variant").unwrap_or_else(|| "tabby".into());
            look.texture = format!("entity/cat/cat_{variant}");
            if tame {
                look.overlays.push(("minecraft:cat#collar".into(), "entity/cat/cat_collar".into(), dye(number(tag, "CollarColor"), 14)));
            }
        }
        "minecraft:rabbit" => {
            let name = match number(tag, "RabbitType").unwrap_or(0) {
                1 => "white",
                2 => "black",
                3 => "white_splotched",
                4 => "gold",
                5 => "salt",
                99 => "caerbannog",
                _ => "brown",
            };
            look.texture = format!("entity/rabbit/rabbit_{name}");
        }
        "minecraft:drowned" => {
            look.overlays.push(("minecraft:drowned#outer".into(), "entity/zombie/drowned_outer_layer".into(), 0xFF_FF_FF));
        }
        "minecraft:villager" => {
            let t = villager_type(tag);
            look.overlays.push((o.layer.into(), format!("entity/villager/type/{t}"), 0xFF_FF_FF));
        }
        "minecraft:zombie_villager" => {
            let t = villager_type(tag);
            look.overlays.push((o.layer.into(), format!("entity/zombie_villager/type/{t}"), 0xFF_FF_FF));
        }
        "minecraft:horse" => {
            let v = number(tag, "Variant").unwrap_or(0);
            let colour = HORSE_COLOURS[((v & 0xFF) as usize).min(6)];
            look.texture = format!("entity/horse/horse_{colour}");
            let mark = MARKINGS[(((v >> 8) & 0xFF) as usize).min(4)];
            if !mark.is_empty() {
                look.overlays.push((o.layer.into(), format!("entity/horse/horse_markings_{mark}"), 0xFF_FF_FF));
            }
        }
        _ => {}
    }
    look
}

/// On the server, a few times a second: what each baby of these kinds looks like.
pub fn scan(host: &mut dyn mod_api::ServerHost, level: mod_api::LevelRef) {
    let mut seen = HashSet::new();
    let mut found = Vec::new();
    for entity in level.all_entities(host).unwrap_or_default() {
        let id = entity.id();
        let Some(kind) = entity.kind(host).map(|key| key.id(host)) else { continue };
        let Some(o) = old(&kind) else { continue };
        let Some(tag) = entity.nbt(host) else { continue };
        if !is_baby(&tag) {
            continue;
        }
        seen.insert(id);
        found.push((id, look_of(o, &tag)));
    }
    // To the players' clients: on a server they draw from these, not from the server's own.
    send(host, level, &found);
    if let Ok(mut looks) = LOOKS.lock() {
        looks.retain(|id, _| seen.contains(id));
        looks.extend(found);
    }
}

/// The grown models, baked once.
static BAKED: LazyLock<Mutex<HashMap<String, ModelPart>>> = LazyLock::new(Default::default);

fn set(root: &mut ModelPart, name: &str, f: impl FnOnce(&mut ModelPart)) {
    if let Some(p) = root.get_child_mut(name) {
        f(p);
    }
}

/// The setupAnim of each kind of model, by its parts' names.
fn pose(root: &mut ModelPart, anim: Anim, mob: &mod_api::MobRender<'_>) {
    root.reset_all_poses();
    let (yaw, pitch) = (mob.head_yaw.to_radians(), mob.pitch.to_radians());
    let pos = mob.walk_position * 0.6662;
    let speed = mob.walk_speed;
    let head = |root: &mut ModelPart| {
        set(root, "head", |p| {
            p.x_rot = pitch;
            p.y_rot = yaw;
        })
    };
    match anim {
        Anim::Quadruped => {
            head(root);
            let swing = pos.cos() * 1.4 * speed;
            for (name, sign) in [("right_hind_leg", 1.0), ("left_front_leg", 1.0), ("left_hind_leg", -1.0), ("right_front_leg", -1.0)] {
                set(root, name, |p| p.x_rot = swing * sign);
            }
        }
        Anim::Chicken => {
            head(root);
            let swing = pos.cos() * 1.4 * speed;
            set(root, "right_leg", |p| p.x_rot = swing);
            set(root, "left_leg", |p| p.x_rot = -swing);
            let flap = if mob.on_ground { 0.0 } else { ((mob.age * 1.3).sin() + 1.0) * 0.6 };
            set(root, "right_wing", |p| p.z_rot = flap);
            set(root, "left_wing", |p| p.z_rot = -flap);
        }
        Anim::Zombie | Anim::Humanoid | Anim::Villager => {
            head(root);
            let legs = if anim == Anim::Villager { 0.5 } else { 1.0 };
            set(root, "right_leg", |p| p.x_rot = pos.cos() * 1.4 * speed * legs);
            set(root, "left_leg", |p| p.x_rot = (pos + PI).cos() * 1.4 * speed * legs);
            match anim {
                // AnimationUtils.animateZombieArms: held out, bobbing.
                Anim::Zombie => {
                    let bob = (mob.age * 0.067).sin() * 0.05;
                    set(root, "right_arm", |p| p.x_rot = -PI / 2.25 + bob);
                    set(root, "left_arm", |p| p.x_rot = -PI / 2.25 - bob);
                }
                Anim::Humanoid => {
                    set(root, "right_arm", |p| p.x_rot = (pos + PI).cos() * speed);
                    set(root, "left_arm", |p| p.x_rot = pos.cos() * speed);
                }
                _ => {}
            }
        }
        Anim::Horse => {
            set(root, "head_parts", |p| p.x_rot = PI / 6.0 + pitch);
            let swing = pos.cos() * 1.4 * speed;
            for (name, sign) in [("right_hind_leg", 1.0), ("left_front_leg", 1.0), ("left_hind_leg", -1.0), ("right_front_leg", -1.0)] {
                set(root, name, |p| p.x_rot = swing * sign);
            }
        }
        Anim::Still => {}
    }
}

/// Shows only the head parts (`head`), or only the rest, anywhere in the tree: a head inside the
/// body (the rabbit's) draws alone in the head pass, its parents' own faces left out.
fn show(part: &mut ModelPart, parts: &[&str], head: bool, in_head: bool) {
    for (name, child) in part.children_mut() {
        let h = in_head || parts.iter().any(|p| name == *p);
        if head {
            child.visible = true;
            child.skip_draw = !h;
        } else {
            child.visible = !h;
        }
        show(child, parts, head, h);
    }
}

/// Each part's `skip_draw`, in the tree's order, to put back after a pass changed them.
fn skips(part: &mut ModelPart, out: &mut Vec<bool>) {
    for (_, child) in part.children_mut() {
        out.push(child.skip_draw);
        skips(child, out);
    }
}

fn restore(part: &mut ModelPart, flags: &mut impl Iterator<Item = bool>) {
    for (_, child) in part.children_mut() {
        child.visible = true;
        child.skip_draw = flags.next().unwrap_or(false);
        restore(child, flags);
    }
}

/// Draws a baby of one of these kinds the old way. Continue for anything else.
pub fn draw(host: &dyn mod_api::ClientHost, event: &mut RenderLivingPreEvent<'_>) -> Flow {
    let mob = event.entity;
    if !mob.baby {
        return Flow::Continue;
    }
    let Some(o) = old(mob.kind) else { return Flow::Continue };
    let look = LOOKS.lock().ok().and_then(|l| l.get(&mob.id).cloned()).unwrap_or_else(|| Look {
        texture: o.texture.to_owned(),
        overlays: Vec::new(),
    });
    // The model and its overlays' models, each with its texture and tint.
    let mut passes: Vec<(String, String, u32)> = vec![(o.layer.to_owned(), look.texture.clone(), 0xFF_FF_FF)];
    passes.extend(look.overlays.iter().cloned());
    let Ok(mut baked) = BAKED.lock() else { return Flow::Continue };
    for (layer, _, _) in &passes {
        if !baked.contains_key(layer) {
            let Some(part) = host.bake_layer(layer) else {
                if layer == o.layer {
                    return Flow::Continue;
                }
                continue;
            };
            baked.insert(layer.clone(), part);
        }
    }
    let hurt = mob.hurt_time > 0 || mob.death_time > 0;
    let draw = &mut *event.draw;
    draw.push();
    if let Shape::Whole(s) = o.shape {
        draw.scale([s, s, s]);
    }
    draw.scale([1.0, -1.0, -1.0]);
    draw.translate([0.0, -1.501, 0.0]);
    for (layer, texture, tint) in &passes {
        let Some(root) = baked.get_mut(layer) else { continue };
        pose(root, o.anim, mob);
        let tint = if hurt { tint & 0xFF_99_99 } else { *tint };
        match o.shape {
            Shape::Whole(_) => {
                draw.model_part(root, texture, 0xFF00_0000 | tint);
            }
            Shape::Split { head_scale, head, body_scale, body_y } => {
                // AgeableListModel.renderToBuffer for the young.
                let mut flags = Vec::new();
                skips(root, &mut flags);
                show(root, o.head, true, false);
                draw.push();
                draw.scale([head_scale, head_scale, head_scale]);
                draw.translate([0.0, head[0] / 16.0, head[1] / 16.0]);
                draw.model_part(root, texture, 0xFF00_0000 | tint);
                draw.pop();
                restore(root, &mut flags.iter().copied());
                show(root, o.head, false, false);
                draw.push();
                draw.scale([body_scale, body_scale, body_scale]);
                draw.translate([0.0, body_y / 16.0, 0.0]);
                draw.model_part(root, texture, 0xFF00_0000 | tint);
                draw.pop();
                restore(root, &mut flags.iter().copied());
            }
        }
    }
    draw.pop();
    Flow::Cancel
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_go_to_the_clients_and_back() {
        let looks = vec![
            (7, Look { texture: "entity/horse/horse_white".into(), overlays: vec![("minecraft:horse".into(), "entity/horse/horse_markings_white".into(), 0xFFFFFF)] }),
            (9, Look { texture: "entity/sheep/sheep".into(), overlays: vec![] }),
        ];
        let messages = encode(&looks);
        assert_eq!(messages.len(), 1);
        assert_eq!(decode(&messages[0]), Some((true, looks)));
    }

    #[test]
    fn many_looks_split_under_the_message_cap() {
        let looks: Vec<(u32, Look)> = (0..2000).map(|i| (i, Look { texture: "entity/villager/villager".into(), overlays: vec![("minecraft:villager".into(), "entity/villager/type/plains".into(), 16777215)] })).collect();
        let messages = encode(&looks);
        assert!(messages.len() > 1 && messages.iter().all(|m| m.len() <= MESSAGE_MAX));
        assert_eq!(messages.iter().filter(|m| m[0] == 1).count(), 1);
        let back: usize = messages.iter().map(|m| decode(m).unwrap().1.len()).sum();
        assert_eq!(back, 2000);
    }
}
