//! Block Carpentry's frames: shapes (a slab, stairs, a door, a fence...) that take on the look of
//! the block put in them.
//!
//! - Right-click a frame with a block: the frame takes on that block's textures, face by face (a
//!   door both halves). The block is used up, except in creative.
//! - Right-click a filled frame with the hammer: it gives the block back and is empty again.
//! - Breaking a filled frame drops the block in it too.
//!
//! A frame's block model is empty; its block entity draws it, every box of its shape (its
//! template's, read from its state) with the frame's own texture while empty, else with the
//! textures of the block in it. The Illusion Block draws the block in it whole, model and all; each
//! other illusion frame is its frame's shape, with the illusion's look while empty.
//!
//! The chests keep a chest's 27 slots (a mod container); the daylight detectors set their power from
//! the sky, as vanilla's (`DaylightDetectorBlock.updateSignalStrength`), for their block entity is
//! this one, not vanilla's.
use mod_api::*;

use crate::groups::FRAMES;

pub const ENTITY: &str = "block_variety_native:frame";
pub const HAMMER: &str = "block_variety:hammer";
const MIMIC: &str = "mimic";
const EMPTY: &str = "block_variety:block/frame";
const ILLUSION: &str = "block_variety:block/illusion";
/// What a tinted face (grass, leaves) is drawn in: a temperate biome's grass.
const GRASS: u32 = 0x91BD59;

pub fn is_frame(block: &str) -> bool {
    FRAMES.contains(&block)
}

/// The shape a frame is drawn as: an illusion frame's is its frame's (`illusion_slab` a
/// `frame_slab`), but for the Illusion Block, which draws its block whole.
fn shape(block: &str) -> String {
    let id = block.split(':').nth(1).unwrap_or("");
    match id.strip_prefix("illusion_") {
        Some(rest) if rest != "block" => format!("frame_{rest}"),
        _ => id.to_owned(),
    }
}

fn is_illusion(block: &str) -> bool {
    block
        .split(':')
        .nth(1)
        .is_some_and(|id| id.starts_with("illusion_"))
}

/// A frame chest's container: a chest's 27 slots, every face reaching them all.
pub fn container(block: &str) -> Option<ModContainer> {
    if !is_frame(block) || shape(block) != "frame_chest" {
        return None;
    }
    let all: Vec<usize> = (0..27).collect();
    Some(ModContainer {
        slots: 27,
        faces: std::array::from_fn(|_| all.clone()),
        inputs: all.clone(),
        outputs: all,
        menu: Some("minecraft:generic_9x3".to_owned()),
    })
}

pub fn ticks(block: &str) -> bool {
    is_frame(block) && shape(block) == "frame_daylight_detector"
}

/// `Level.getTimeOfDay` for a day time in ticks: 0 to 1, 0.25 at noon... as vanilla's curve.
fn time_of_day(day_time: i64) -> f32 {
    let d = ((day_time.rem_euclid(24000)) as f64 / 24000.0 - 0.25).rem_euclid(1.0);
    let e = 0.5 - (d * std::f64::consts::PI).cos() / 2.0;
    ((d * 2.0 + e) / 3.0) as f32
}

/// A daylight detector's power, as `DaylightDetectorBlock.updateSignalStrength`: the sky light less
/// the sky's darkening, by the sun's angle; inverted, what is left of 15.
fn daylight_power(sky: u8, day_time: i64, rain: f32, thunder: f32, inverted: bool) -> u8 {
    let tod = time_of_day(day_time);
    let mut g = 1.0 - ((tod * std::f32::consts::TAU).cos() * 2.0 + 0.5);
    g = 1.0 - g.clamp(0.0, 1.0);
    g *= 1.0 - rain * 5.0 / 16.0;
    g *= 1.0 - thunder * 5.0 / 16.0;
    let darken = ((1.0 - g) * 11.0) as i32;
    let mut i = i32::from(sky) - darken;
    if inverted {
        i = 15 - i;
    } else if i > 0 {
        let mut f = tod * std::f32::consts::TAU;
        let to = if f < std::f32::consts::PI {
            0.0
        } else {
            std::f32::consts::TAU
        };
        f += (to - f) * 0.2;
        i = (i as f32 * f.cos()).round() as i32;
    }
    i.clamp(0, 15) as u8
}

/// A frame daylight detector, ticked: its power from the sky, every second.
pub fn tick(host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
    *event.reply = BlockEntityTickReply::Unchanged;
    let Some(level) = host.level(host.dimension()) else {
        return;
    };
    let Some(day) = level.day_time(host) else {
        return;
    };
    if day % 20 != 0 {
        return;
    }
    let sky = level
        .brightness(host, LightLayer::Sky, event.pos)
        .unwrap_or(0) as u8;
    let (rain, thunder) = level.weather(host).unwrap_or_default();
    let inverted = level.block_property(host, event.pos, "inverted").as_deref() == Some("true");
    let power = daylight_power(sky, day, rain, thunder, inverted);
    host.set_block_property(event.pos, "power", &power.to_string());
}

/// The block in a frame's compound, if any.
pub fn mimic(nbt: &Compound) -> Option<String> {
    match nbt.get(&MIMIC.into()) {
        Some(Tag::String(s)) => s.to_string().ok(),
        _ => None,
    }
}

type Box6 = [f32; 6];

/// `b` (texels, as a model element) turned about the block's upright axis from facing north to
/// `facing`, clockwise seen from above.
fn turn(b: Box6, facing: &str) -> Box6 {
    let quarter = match facing {
        "east" => 1,
        "south" => 2,
        "west" => 3,
        _ => 0,
    };
    let (mut x0, mut z0, mut x1, mut z1) = (b[0], b[2], b[3], b[5]);
    for _ in 0..quarter {
        (x0, z0, x1, z1) = (16.0 - z1, x0, 16.0 - z0, x1);
    }
    [x0.min(x1), b[1], z0.min(z1), x0.max(x1), b[4], z0.max(z1)]
}

/// The boxes a frame of shape `kind` is drawn as in state `prop` (its template's properties).
fn boxes(kind: &str, prop: &dyn Fn(&str) -> Option<String>) -> Vec<Box6> {
    let p = |k: &str| prop(k).unwrap_or_default();
    let is = |k: &str| p(k) == "true";
    let facing = p("facing");
    match kind {
        "frame_slab" => match p("type").as_str() {
            "top" => vec![[0.0, 8.0, 0.0, 16.0, 16.0, 16.0]],
            "double" => vec![[0.0, 0.0, 0.0, 16.0, 16.0, 16.0]],
            _ => vec![[0.0, 0.0, 0.0, 16.0, 8.0, 16.0]],
        },
        "frame_stairs" => {
            let top = p("half") == "top";
            let (base, step) = if top {
                ((8.0, 16.0), (0.0, 8.0))
            } else {
                ((0.0, 8.0), (8.0, 16.0))
            };
            // Facing north: the step is on the north side; left is west.
            let (nw, ne, sw, se) = (
                [0.0, 0.0, 8.0, 8.0],
                [8.0, 0.0, 16.0, 8.0],
                [0.0, 8.0, 8.0, 16.0],
                [8.0, 8.0, 16.0, 16.0],
            );
            let quarters: Vec<[f32; 4]> = match p("shape").as_str() {
                "outer_left" => vec![nw],
                "outer_right" => vec![ne],
                "inner_left" => vec![nw, ne, sw],
                "inner_right" => vec![nw, ne, se],
                _ => vec![nw, ne],
            };
            let mut out = vec![[0.0, base.0, 0.0, 16.0, base.1, 16.0]];
            out.extend(
                quarters
                    .iter()
                    .map(|q| turn([q[0], step.0, q[1], q[2], step.1, q[3]], &facing)),
            );
            out
        }
        "frame_wall" => {
            let mut out = Vec::new();
            if is("up") {
                out.push([4.0, 0.0, 4.0, 12.0, 16.0, 12.0]);
            }
            for side in ["north", "east", "south", "west"] {
                let h = match p(side).as_str() {
                    "low" => 14.0,
                    "tall" => 16.0,
                    _ => continue,
                };
                out.push(turn([5.0, 0.0, 0.0, 11.0, h, 8.0], side));
            }
            out
        }
        "frame_fence" => {
            let mut out = vec![[6.0, 0.0, 6.0, 10.0, 16.0, 10.0]];
            for side in ["north", "east", "south", "west"] {
                if is(side) {
                    out.push(turn([7.0, 12.0, 0.0, 9.0, 15.0, 6.0], side));
                    out.push(turn([7.0, 6.0, 0.0, 9.0, 9.0, 6.0], side));
                }
            }
            out
        }
        "frame_pane" => {
            let mut out = vec![[7.0, 0.0, 7.0, 9.0, 16.0, 9.0]];
            for side in ["north", "east", "south", "west"] {
                if is(side) {
                    out.push(turn([7.0, 0.0, 0.0, 9.0, 16.0, 7.0], side));
                }
            }
            out
        }
        "frame_fence_gate" => {
            let drop = if is("in_wall") { 3.0 } else { 0.0 };
            let mut out: Vec<Box6> = vec![
                [0.0, 5.0, 7.0, 2.0, 16.0, 9.0],
                [14.0, 5.0, 7.0, 16.0, 16.0, 9.0],
            ];
            if is("open") {
                for x in [0.0, 14.0] {
                    out.push([x, 6.0, 1.0, x + 2.0, 15.0, 3.0]);
                    out.push([x, 6.0, 3.0, x + 2.0, 9.0, 7.0]);
                    out.push([x, 12.0, 3.0, x + 2.0, 15.0, 7.0]);
                }
            } else {
                out.extend([
                    [6.0, 6.0, 7.0, 8.0, 15.0, 9.0],
                    [8.0, 6.0, 7.0, 10.0, 15.0, 9.0],
                    [2.0, 6.0, 7.0, 6.0, 9.0, 9.0],
                    [2.0, 12.0, 7.0, 6.0, 15.0, 9.0],
                    [10.0, 6.0, 7.0, 14.0, 9.0, 9.0],
                    [10.0, 12.0, 7.0, 14.0, 15.0, 9.0],
                ]);
            }
            out.into_iter()
                .map(|b| turn([b[0], b[1] - drop, b[2], b[3], b[4] - drop, b[5]], &facing))
                .collect()
        }
        "frame_door" => {
            // As vanilla's `DoorBlock.getShape`.
            let (s, n, w, e) = (
                [0.0, 0.0, 0.0, 16.0, 16.0, 3.0],
                [0.0, 0.0, 13.0, 16.0, 16.0, 16.0],
                [13.0, 0.0, 0.0, 16.0, 16.0, 16.0],
                [0.0, 0.0, 0.0, 3.0, 16.0, 16.0],
            );
            let closed = !is("open");
            let right = p("hinge") == "right";
            vec![match facing.as_str() {
                "south" => {
                    if closed {
                        s
                    } else if right {
                        e
                    } else {
                        w
                    }
                }
                "west" => {
                    if closed {
                        w
                    } else if right {
                        s
                    } else {
                        n
                    }
                }
                "north" => {
                    if closed {
                        n
                    } else if right {
                        w
                    } else {
                        e
                    }
                }
                _ => {
                    if closed {
                        e
                    } else if right {
                        n
                    } else {
                        s
                    }
                }
            }]
        }
        "frame_trapdoor" => vec![if is("open") {
            match facing.as_str() {
                "south" => [0.0, 0.0, 0.0, 16.0, 16.0, 3.0],
                "west" => [13.0, 0.0, 0.0, 16.0, 16.0, 16.0],
                "east" => [0.0, 0.0, 0.0, 3.0, 16.0, 16.0],
                _ => [0.0, 0.0, 13.0, 16.0, 16.0, 16.0],
            }
        } else if p("half") == "top" {
            [0.0, 13.0, 0.0, 16.0, 16.0, 16.0]
        } else {
            [0.0, 0.0, 0.0, 16.0, 3.0, 16.0]
        }],
        "frame_ladder" => vec![turn([0.0, 0.0, 15.0, 16.0, 16.0, 16.0], &facing)],
        "frame_chest" => vec![[1.0, 0.0, 1.0, 15.0, 14.0, 15.0]],
        "frame_daylight_detector" => vec![[0.0, 0.0, 0.0, 16.0, 6.0, 16.0]],
        "frame_bed" => {
            // Facing north, the head is the north half; each half has its legs at its far end.
            let legs_at = if p("part") == "head" { 0.0 } else { 13.0 };
            let mut out = vec![[0.0, 3.0, 0.0, 16.0, 9.0, 16.0]];
            for x in [0.0, 13.0] {
                out.push(turn(
                    [x, 0.0, legs_at, x + 3.0, 3.0, legs_at + 3.0],
                    &facing,
                ));
            }
            out
        }
        "frame_carpet" => vec![[0.0, 0.0, 0.0, 16.0, 1.0, 16.0]],
        "frame_pressure_plate" => vec![[
            1.0,
            0.0,
            1.0,
            15.0,
            if is("powered") { 0.5 } else { 1.0 },
            15.0,
        ]],
        "frame_layered_block" => {
            let n: f32 = p("layers").parse().unwrap_or(1.0);
            vec![[0.0, 0.0, 0.0, 16.0, 2.0 * n, 16.0]]
        }
        "frame_button" => {
            let h = if is("powered") { 1.0 } else { 2.0 };
            match p("face").as_str() {
                "floor" => vec![turn([5.0, 0.0, 6.0, 11.0, h, 10.0], &facing)],
                "ceiling" => vec![turn([5.0, 16.0 - h, 6.0, 11.0, 16.0, 10.0], &facing)],
                // On a wall facing north, the button is on the block to the south's face.
                _ => vec![turn([5.0, 6.0, 16.0 - h, 11.0, 10.0, 16.0], &facing)],
            }
        }
        _ => vec![[0.0, 0.0, 0.0, 16.0, 16.0, 16.0]],
    }
}

/// The sprite and tint of each side: the block in the frame's, else the frame's own.
fn faces(
    host: &dyn ClientHost,
    mimic: Option<BlockState>,
    empty: &str,
) -> [(String, Option<u32>); 6] {
    let sprites = mimic.and_then(|state| host.block_sprites(&[state]).into_iter().next().flatten());
    std::array::from_fn(|side| {
        match sprites
            .as_ref()
            .and_then(|sprites| sprites.sides[side].first())
        {
            Some(face) => (
                face.texture.clone(),
                (face.tint_index >= 0).then_some(GRASS),
            ),
            None => (empty.to_owned(), None),
        }
    })
}

fn draw_box(draw: &mut dyn BlockEntityDrawContext, b: Box6, faces: &[(String, Option<u32>); 6]) {
    // The untinted faces in one box, each tinted face on its own.
    let plain: [Option<CubeFace<'_>>; 6] =
        std::array::from_fn(|i| faces[i].1.is_none().then(|| CubeFace::new(&faces[i].0)));
    if plain.iter().any(Option::is_some) {
        draw.cube([b[0], b[1], b[2]], [b[3], b[4], b[5]], plain, 0xFF_FF_FF);
    }
    for i in 0..6 {
        if let Some(tint) = faces[i].1 {
            let one: [Option<CubeFace<'_>>; 6] =
                std::array::from_fn(|j| (j == i).then(|| CubeFace::new(&faces[i].0)));
            draw.cube([b[0], b[1], b[2]], [b[3], b[4], b[5]], one, tint);
        }
    }
}

/// A slope: a wedge, high on its facing side, as Block Carpentry's (an upside-down one when its
/// half is the top). Each face both ways round, so the winding can't hide it.
fn draw_slope(
    draw: &mut dyn BlockEntityDrawContext,
    facing: &str,
    top: bool,
    faces: &[(String, Option<u32>); 6],
) {
    let yaw = match facing {
        "west" => 90.0,
        "south" => 180.0,
        "east" => 270.0,
        _ => 0.0,
    };
    draw.push();
    draw.rotate_about([0.0, 1.0, 0.0], yaw, [0.5, 0.0, 0.5]);
    let y = |v: f32| if top { 1.0 - v } else { v };
    let (bottom, up, back, side) = (
        &faces[if top { 1 } else { 0 }],
        &faces[1],
        &faces[2],
        &faces[4],
    );
    let quad =
        |draw: &mut dyn BlockEntityDrawContext, face: &(String, Option<u32>), c: [[f32; 3]; 4]| {
            let tint = face.1.unwrap_or(0xFF_FF_FF);
            draw.quad(&face.0, c, [0.0, 0.0, 16.0, 16.0], tint);
            draw.quad(
                &face.0,
                [c[3], c[2], c[1], c[0]],
                [16.0, 0.0, 0.0, 16.0],
                tint,
            );
        };
    // Under it, its back (north) and its slanted top, from the back's top edge to the front's foot.
    quad(
        draw,
        bottom,
        [
            [0.0, y(0.0), 1.0],
            [0.0, y(0.0), 0.0],
            [1.0, y(0.0), 0.0],
            [1.0, y(0.0), 1.0],
        ],
    );
    quad(
        draw,
        back,
        [
            [1.0, y(1.0), 0.0],
            [1.0, y(0.0), 0.0],
            [0.0, y(0.0), 0.0],
            [0.0, y(1.0), 0.0],
        ],
    );
    quad(
        draw,
        up,
        [
            [0.0, y(1.0), 0.0],
            [0.0, y(0.0), 1.0],
            [1.0, y(0.0), 1.0],
            [1.0, y(1.0), 0.0],
        ],
    );
    // Its sides, triangles: a quad with two corners in one place.
    for x in [0.0, 1.0] {
        quad(
            draw,
            side,
            [
                [x, y(1.0), 0.0],
                [x, y(0.0), 0.0],
                [x, y(0.0), 1.0],
                [x, y(0.0), 1.0],
            ],
        );
    }
    draw.pop();
}

/// The heights (0 or 1) of a slope's top at its NW, NE, SE and SW corners, facing north, by its
/// stairs shape: straight, high along the north; an outer corner, high at one corner; an inner
/// corner, low at one.
fn edged_heights(shape: &str) -> [f32; 4] {
    match shape {
        "outer_left" => [1.0, 0.0, 0.0, 0.0],
        "outer_right" => [0.0, 1.0, 0.0, 0.0],
        "inner_left" => [1.0, 1.0, 0.0, 1.0],
        "inner_right" => [1.0, 1.0, 1.0, 0.0],
        _ => [1.0, 1.0, 0.0, 0.0],
    }
}

/// An edged slope: its top a surface over its corners' heights, split corner to corner along the
/// diagonal whose ends are level, and its sides down to its foot. Each face both ways round.
fn draw_edged(
    draw: &mut dyn BlockEntityDrawContext,
    facing: &str,
    top: bool,
    shape: &str,
    faces: &[(String, Option<u32>); 6],
) {
    let yaw = match facing {
        "west" => 90.0,
        "south" => 180.0,
        "east" => 270.0,
        _ => 0.0,
    };
    let h = edged_heights(shape);
    draw.push();
    draw.rotate_about([0.0, 1.0, 0.0], yaw, [0.5, 0.0, 0.5]);
    let y = |v: f32| if top { 1.0 - v } else { v };
    // NW, NE, SE, SW at their heights, and at the foot.
    let xz = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let up = |i: usize| [xz[i][0], y(h[i]), xz[i][1]];
    let foot = |i: usize| [xz[i][0], y(0.0), xz[i][1]];
    let quad =
        |draw: &mut dyn BlockEntityDrawContext, face: &(String, Option<u32>), c: [[f32; 3]; 4]| {
            let tint = face.1.unwrap_or(0xFF_FF_FF);
            draw.quad(&face.0, c, [0.0, 0.0, 16.0, 16.0], tint);
            draw.quad(
                &face.0,
                [c[3], c[2], c[1], c[0]],
                [16.0, 0.0, 0.0, 16.0],
                tint,
            );
        };
    let (bottom, upf) = (&faces[if top { 1 } else { 0 }], &faces[1]);
    quad(draw, bottom, [foot(3), foot(0), foot(1), foot(2)]);
    // The top: two triangles (quads with a corner twice).
    let (a, b, c, d) = if h[1] == h[3] {
        (0, 1, 2, 3)
    } else {
        (1, 2, 3, 0)
    };
    quad(draw, upf, [up(a), up(b), up(d), up(d)]);
    quad(draw, upf, [up(b), up(c), up(d), up(d)]);
    // The sides: north, east, south, west, from each corner to the next.
    for (i, side) in [(0usize, 2usize), (1, 5), (2, 3), (3, 4)] {
        let j = (i + 1) % 4;
        if h[i] == 0.0 && h[j] == 0.0 {
            continue;
        }
        quad(draw, &faces[side], [up(i), foot(i), foot(j), up(j)]);
    }
    draw.pop();
}

pub fn update_tag(event: &BlockEntityUpdateTagEvent<'_>) -> Option<Compound> {
    (event.entity.id() == ENTITY).then(|| event.entity.nbt.clone())
}

pub fn submit(host: &dyn ClientHost, event: &mut BlockEntitySubmitEvent<'_>) {
    if !is_frame(event.block) {
        return;
    }
    let kind = shape(event.block);
    let kind = kind.as_str();
    let empty = if is_illusion(event.block) {
        ILLUSION
    } else {
        EMPTY
    };
    let state = event.state;
    let filled = mimic(&event.entity.nbt);
    let mimic_state = filled.as_deref().and_then(|m| host.block_state_id(m));
    let draw = &mut *event.draw;
    if kind == "illusion_block" {
        match &filled {
            Some(m) if mimic_state.is_some() => {
                if let Some(block) = host.block_key(m) {
                    draw.block(block, &[]);
                }
            }
            _ => draw_box(
                draw,
                [0.0, 0.0, 0.0, 16.0, 16.0, 16.0],
                &faces(host, None, ILLUSION),
            ),
        }
        draw.keep();
        return;
    }
    let faces = faces(host, mimic_state, empty);
    let prop = |k: &str| {
        state
            .property(&host.block_states(), k)
            .ok()
            .flatten()
            .map(str::to_owned)
    };
    if kind == "frame_slope" {
        draw_slope(
            draw,
            &prop("facing").unwrap_or_default(),
            prop("half").as_deref() == Some("top"),
            &faces,
        );
    } else if kind == "frame_edged_slope" {
        let shape = prop("shape").unwrap_or_default();
        draw_edged(
            draw,
            &prop("facing").unwrap_or_default(),
            prop("half").as_deref() == Some("top"),
            &shape,
            &faces,
        );
    } else {
        for b in boxes(kind, &prop) {
            draw_box(draw, b, &faces);
        }
    }
    draw.keep();
}

/// The other half of a door or a bed at `pos` in state string `state`, if it is one.
fn other_half(pos: [i32; 3], state: &str) -> Option<[i32; 3]> {
    let block = state.split('[').next().unwrap_or(state);
    let [x, y, z] = pos;
    match shape(block).as_str() {
        "frame_door" => Some(if state.contains("half=upper") {
            [x, y - 1, z]
        } else {
            [x, y + 1, z]
        }),
        "frame_bed" => {
            // A bed faces from its foot to its head.
            let (dx, dz) = if state.contains("facing=south") {
                (0, 1)
            } else if state.contains("facing=west") {
                (-1, 0)
            } else if state.contains("facing=east") {
                (1, 0)
            } else {
                (0, -1)
            };
            let s = if state.contains("part=head") { -1 } else { 1 };
            Some([x + dx * s, y, z + dz * s])
        }
        _ => None,
    }
}

fn set_mimic(
    host: &mut dyn ServerHost,
    entity: BlockEntityRef,
    value: Option<&str>,
) -> Result<(), ()> {
    let mut data = entity.data(host).ok_or(())?;
    if let Some(value) = value {
        data.insert(MIMIC.into(), Tag::String(value.into()));
    } else {
        data.remove(&MIMIC.into());
    }
    entity.replace_data(host, data).map_err(|_| ())?;
    Ok(())
}

pub fn click(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    level: LevelRef,
    here: &str,
    click: &BlockClick<'_>,
) -> bool {
    let Some(stack) = click.stack else {
        return false;
    };
    let Some(entity) = host.block_entity(level, click.pos) else {
        return false;
    };
    let Some(data) = entity.data(host) else {
        return false;
    };
    let filled = mimic(&data);
    let item = stack.item().id(host);
    let hammer = item == HAMMER;
    let creative = player.game_mode(host) == Some(GameMode::Creative);
    if (hammer && filled.is_none())
        || (!hammer
            && (filled.is_some()
                || item == "minecraft:air"
                || is_frame(&item)
                || host.block_state_id(&item).is_none()))
    {
        return false;
    }
    let entities = std::iter::once(entity)
        .chain(other_half(click.pos, here).and_then(|pos| host.block_entity(level, pos)))
        .collect::<Vec<_>>();
    let changed = host
        .transaction(|host, _| -> Result<(), ()> {
            for entity in entities {
                set_mimic(host, entity, (!hammer).then_some(item.as_str()))?;
            }
            if !creative {
                if hammer {
                    let filled = filled.as_deref().ok_or(())?;
                    let stack = ItemStack::of(super::item_key(host, filled).ok_or(())?);
                    player.give_stack(host, stack).map_err(|_| ())?;
                } else {
                    let slot = if click.hand == Hand::Off {
                        40
                    } else {
                        player.selected_slot(host).ok_or(())? as u32
                    };
                    let next = stack.clone().shrink(1);
                    player
                        .set_inventory_slot(host, slot, next)
                        .map_err(|_| ())?;
                }
            }
            Ok(())
        })
        .is_ok();
    if changed {
        let sound = if hammer {
            "minecraft:block.wood.break"
        } else {
            "minecraft:block.wood.place"
        };
        level.play_sound(
            host,
            sound,
            click.pos.map(|v| f64::from(v) + 0.5),
            SoundSource::Blocks,
            1.0,
            1.0,
        );
    }
    changed
}

pub fn destroy_block(host: &mut dyn ServerHost, event: &BreakBlockEvent<'_>) {
    let Some(player) = host.players().acting_player() else {
        return;
    };
    if player.game_mode(host) == Some(GameMode::Creative) {
        return;
    }
    let Some(level) = player.level(host) else {
        return;
    };
    let Some(entity) = host.block_entity(level, event.pos) else {
        return;
    };
    let Some(data) = entity.data(host) else {
        return;
    };
    let Some(filled) = mimic(&data) else { return };
    let Some(key) = super::item_key(host, &filled) else {
        return;
    };
    let [x, y, z] = event.pos.map(|v| f64::from(v) + 0.5);
    let _ = level.spawn_item(host, x, y, z, ItemStack::of(key), 10);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(list: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |k| {
            list.iter()
                .find(|(a, _)| *a == k)
                .map(|(_, v)| (*v).to_owned())
        }
    }

    #[test]
    fn turning_goes_round_clockwise() {
        let north = [0.0, 0.0, 0.0, 16.0, 16.0, 3.0];
        assert_eq!(turn(north, "east"), [13.0, 0.0, 0.0, 16.0, 16.0, 16.0]);
        assert_eq!(turn(north, "south"), [0.0, 0.0, 13.0, 16.0, 16.0, 16.0]);
        assert_eq!(turn(north, "west"), [0.0, 0.0, 0.0, 3.0, 16.0, 16.0]);
    }

    #[test]
    fn stairs_and_slabs_have_their_shapes() {
        let straight = boxes(
            "frame_stairs",
            &props(&[
                ("facing", "north"),
                ("half", "bottom"),
                ("shape", "straight"),
            ]),
        );
        assert_eq!(
            straight,
            vec![
                [0.0, 0.0, 0.0, 16.0, 8.0, 16.0],
                [0.0, 8.0, 0.0, 8.0, 16.0, 8.0],
                [8.0, 8.0, 0.0, 16.0, 16.0, 8.0]
            ]
        );
        let inner = boxes(
            "frame_stairs",
            &props(&[("facing", "east"), ("half", "top"), ("shape", "inner_left")]),
        );
        assert_eq!(inner.len(), 4);
        assert_eq!(inner[0], [0.0, 8.0, 0.0, 16.0, 16.0, 16.0]);
        assert_eq!(
            boxes("frame_slab", &props(&[("type", "double")])),
            vec![[0.0, 0.0, 0.0, 16.0, 16.0, 16.0]]
        );
    }

    #[test]
    fn every_frame_draws_something() {
        for f in FRAMES {
            let kind = shape(f);
            if kind == "frame_slope" || kind == "frame_edged_slope" || kind == "illusion_block" {
                continue;
            }
            let b = boxes(
                &kind,
                &props(&[
                    ("facing", "north"),
                    ("up", "true"),
                    ("layers", "3"),
                    ("face", "wall"),
                ]),
            );
            assert!(!b.is_empty(), "{f}");
            for x in &b {
                assert!(
                    (0..3).all(|i| x[i] <= x[i + 3] && x[i] >= 0.0 && x[i + 3] <= 16.0),
                    "{f} {x:?}"
                );
            }
        }
    }

    #[test]
    fn a_door_is_two_halves() {
        assert_eq!(
            other_half(
                [0, 5, 0],
                "block_variety:frame_door[half=upper,facing=north]"
            ),
            Some([0, 4, 0])
        );
        assert_eq!(
            other_half([0, 5, 0], "block_variety:frame_door[half=lower]"),
            Some([0, 6, 0])
        );
        assert_eq!(
            other_half([0, 5, 0], "block_variety:frame_slab[type=top]"),
            None
        );
        assert_eq!(
            other_half([0, 5, 0], "block_variety:illusion_door[half=lower]"),
            Some([0, 6, 0])
        );
    }

    #[test]
    fn a_bed_is_two_halves() {
        assert_eq!(
            other_half([0, 5, 0], "block_variety:frame_bed[facing=north,part=foot]"),
            Some([0, 5, -1])
        );
        assert_eq!(
            other_half(
                [0, 5, -1],
                "block_variety:frame_bed[facing=north,part=head]"
            ),
            Some([0, 5, 0])
        );
        assert_eq!(
            other_half(
                [3, 5, 0],
                "block_variety:illusion_bed[facing=east,part=head]"
            ),
            Some([2, 5, 0])
        );
    }

    #[test]
    fn illusions_take_their_frames_shape() {
        assert_eq!(shape("block_variety:illusion_slab"), "frame_slab");
        assert_eq!(shape("block_variety:illusion_block"), "illusion_block");
        assert_eq!(shape("block_variety:frame_chest"), "frame_chest");
        assert!(
            is_illusion("block_variety:illusion_wall") && !is_illusion("block_variety:frame_wall")
        );
    }

    #[test]
    fn daylight_is_strong_at_noon_and_gone_at_midnight() {
        assert_eq!(daylight_power(15, 6000, 0.0, 0.0, false), 15);
        assert_eq!(daylight_power(15, 18000, 0.0, 0.0, false), 0);
        assert_eq!(daylight_power(15, 18000, 0.0, 0.0, true), 11);
        assert_eq!(daylight_power(0, 6000, 0.0, 0.0, false), 0);
    }

    #[test]
    fn edged_slopes_turn_corners() {
        assert_eq!(edged_heights("straight"), [1.0, 1.0, 0.0, 0.0]);
        assert_eq!(edged_heights("outer_left").iter().sum::<f32>(), 1.0);
        assert_eq!(edged_heights("inner_right").iter().sum::<f32>(), 3.0);
    }
}
