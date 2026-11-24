//! The overlay: ghost blocks, the verifier's boxes, the highlights and the frames (placements, the
//! area selection, a save's missing chunks).
//!
//! The mod API has no way to draw this much geometry each frame or to read the game's full block
//! models, so the overlay goes through the native route:
//!
//! - The game side (`src/native/game_support.rs`, a support module in the game's `game` module)
//!   runs `tick` once per client tick with `GameOps`: block models, the white texel's UV and a
//!   token per world section that changes whenever the section does.
//! - `tick` builds quads per 16x16x16 section into `SCENE`.
//! - The render side (`src/native/world_drawn.rs`, a mixin on `horizon_mod_render`'s
//!   `world_drawn`) uploads changed sections to the GPU and draws them over the world, in a pass
//!   of its own. It keeps its GPU objects in `GPU` (their type is its own; this crate has no wgpu).
//!
//! A quad is 4 `Vertex`es, positions relative to `Scene::anchor`. `normal.w = 127` marks a quad
//! with an outward normal: it is dropped while it faces away from the camera, so a ghost shows
//! only its near faces.

use crate::runtime::{self, Grab, Resolved};
use crate::settings::Mode;
use crate::world::{StateId, World};
use horizonmatica_core::verify::Status;
use mod_api::ClientHost;
use std::any::Any;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------- what the game side gives

/// One vertex, laid out as the game's entity vertex (`horizon_mod_render::render::Vertex`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub layer: u32,
    /// RGBA8.
    pub color: u32,
    /// Sky in bits 0..8, block in bits 8..16 (16x fixed point).
    pub light: u32,
    pub normal: [i8; 4],
    pub glint_uv: [f32; 2],
}

/// The vertices as bytes, for the GPU.
pub fn vertex_bytes(v: &[Vertex]) -> &[u8] {
    // SAFETY: `Vertex` is `repr(C)` of 4-byte fields only (no padding), every bit pattern of
    // which is a valid byte.
    unsafe { std::slice::from_raw_parts(v.as_ptr().cast::<u8>(), std::mem::size_of_val(v)) }
}

/// What the overlay needs of the game beyond the mod API, given by the game side each tick.
pub trait GameOps {
    /// The atlas UV of a white texel (for the boxes and frames).
    fn white_uv(&self) -> [f32; 2];
    /// A token for the client world's section `k` (chunk x, section y, chunk z): the same
    /// object while the section is unchanged, another once a block in it changed or its chunk
    /// came again. None where the client holds none.
    fn section(&self, k: [i32; 3]) -> Option<Arc<dyn Any + Send + Sync>>;
    /// The baked model of `state` moved by `at`, lit `light`, with each face's own normal and no
    /// directional shade.
    fn block_model(&self, state: StateId, at: [f32; 3], light: u32, out: &mut Vec<Vertex>);
    /// The block entity model of `state` (chests, banners, heads...), moved by `at`.
    fn special_model(&self, state: StateId, at: [f32; 3], out: &mut Vec<Vertex>);
    /// Whether `state` is a full opaque cube (`solid_render`).
    fn solid_cube(&self, state: StateId) -> bool;
}

// ---------------------------------------------------------------- the scene (shared with the render side)

/// What the render side draws: quads per section, each section replaced on its own.
#[derive(Default)]
pub struct Scene {
    /// World block position the vertex positions are relative to (shared by every section).
    pub anchor: [i32; 3],
    /// Per section: its revision and quads. Once uploaded the quads are let go (the GPU keeps
    /// them), so they take memory once, not twice.
    pub sections: HashMap<[i32; 3], (u64, Vec<Vertex>)>,
    /// The highlights, per section in the same way (drawn after the rest, through everything).
    pub marks: HashMap<[i32; 3], (u64, Vec<Vertex>)>,
    /// Set by the render side when it made its GPU objects anew after quads were let go: the
    /// game side builds every section again (`take_lost`).
    pub lost: bool,
    /// Bumped on every change.
    pub revision: u64,
    /// Draw through terrain (no test against the world's depth).
    pub through: bool,
    pub visible: bool,
}

pub static SCENE: Mutex<Option<Scene>> = Mutex::new(None);

/// The render side's GPU objects (a type of its own).
pub static GPU: Mutex<Option<Box<dyn Any + Send>>> = Mutex::new(None);

/// Whether the render side needs the world's depth this frame (`render_needs`).
pub fn wants_world_depth() -> bool {
    let s = SCENE.lock().unwrap_or_else(|e| e.into_inner());
    s.as_ref().is_some_and(|s| s.visible && !s.through && !s.sections.is_empty())
}

fn scene<R>(f: impl FnOnce(&mut Scene) -> R) -> R {
    let mut s = SCENE.lock().unwrap_or_else(|e| e.into_inner());
    f(s.get_or_insert_with(Scene::default))
}

/// Sets the anchor; a new one drops every section (their positions were relative to the old).
fn set_anchor(anchor: [i32; 3]) {
    scene(|s| {
        if s.anchor != anchor {
            s.anchor = anchor;
            s.sections.clear();
            s.marks.clear();
            s.revision += 1;
        }
    });
}

/// Replaces section `key`'s quads. Empty removes it.
fn put_section(key: [i32; 3], quads: Vec<Vertex>) {
    scene(|s| {
        s.revision += 1;
        if quads.is_empty() {
            s.sections.remove(&key);
        } else {
            let rev = s.revision;
            s.sections.insert(key, (rev, quads));
        }
    });
}

fn remove_section(key: [i32; 3]) {
    scene(|s| {
        if s.sections.remove(&key).is_some() {
            s.revision += 1;
        }
    });
}

/// Replaces section `key`'s highlights. Empty removes them.
fn put_marks(key: [i32; 3], quads: Vec<Vertex>) {
    scene(|s| {
        s.revision += 1;
        if quads.is_empty() {
            s.marks.remove(&key);
        } else {
            let rev = s.revision;
            s.marks.insert(key, (rev, quads));
        }
    });
}

fn clear_marks() {
    scene(|s| {
        s.marks.clear();
        s.revision += 1;
    });
}

fn take_lost() -> bool {
    scene(|s| std::mem::take(&mut s.lost))
}

/// Changes only the draw options, keeping the uploaded geometry.
fn options(through: bool, visible: bool) {
    scene(|s| {
        s.through = through;
        s.visible = visible;
    });
}

/// Drops every section. The revision keeps counting: the render side keeps a buffer while its
/// section's revision matches, so starting again from 0 would keep showing the old quads.
fn clear_scene() {
    scene(|s| {
        s.sections.clear();
        s.marks.clear();
        s.revision += 1;
    });
}

// ---------------------------------------------------------------- the game side's bookkeeping

/// A section token as last seen (`GameOps::section`).
type Snap = Option<Arc<dyn Any + Send + Sync>>;

fn same(a: &Snap, b: &Snap) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

/// What the game side remembers of what it handed over.
#[derive(Default)]
struct Mesh {
    /// What every section shows (placements, layers, display settings): another one builds all
    /// of them again.
    key: Option<MeshKey>,
    anchor: [i32; 3],
    /// Built sections: the world section each was built against and its quad count.
    built: HashMap<[i32; 3], (Snap, usize)>,
    quads: usize,
    /// The far sections found stale, nearest first: the ticks in between build on from here
    /// once the near ones are done.
    queue: VecDeque<[i32; 3]>,
    tick: u64,
    /// The frames as last handed over.
    area: Option<([Option<[i32; 3]>; 2], [i32; 3], Option<Grab>)>,
    save: Option<(Vec<(i32, i32)>, [i32; 3])>,
    frames: Option<(Vec<([i32; 3], [i32; 3], u32, u32)>, [i32; 3])>,
    /// What the highlights show now: missing, wrong, only this item.
    marks_key: Option<(bool, bool, Option<String>)>,
    marks: HashMap<[i32; 3], (Snap, usize)>,
    marks_count: usize,
    marks_queue: VecDeque<[i32; 3]>,
    /// Full cube or not, per game state.
    cubes: HashMap<StateId, bool>,
}

type MeshKey = (u64, horizonmatica_core::LayerMode, Mode, bool, bool, bool, bool, u32, bool);

thread_local! {
    static MESH: RefCell<Mesh> = RefCell::new(Mesh::default());
}

fn mesh<R>(f: impl FnOnce(&mut Mesh) -> R) -> R {
    MESH.with_borrow_mut(f)
}

/// Fully lit (sky 15, block 15 as 16x fixed point): ghosts show their texture's own colours.
const FULL_BRIGHT: u32 = 240 | (240 << 8);
/// The sections within this many blocks are checked every tick, the rest every `FAR_EVERY`.
const NEAR: i32 = 48;
const FAR_EVERY: u64 = 10;
/// Time the overlay may take per tick to build sections, and the highlights.
const MESH_BUDGET: Duration = Duration::from_millis(4);
const MARKS_BUDGET: Duration = Duration::from_millis(3);
/// Most blocks highlighted at once, the nearest ones.
const MAX_MARKS: usize = 50_000;
/// Most chunk frames a save shows.
const SAVE_FRAMES: usize = 1024;

/// The scene's keys for the frames (no real section is that far out).
const AREA_SECTION: [i32; 3] = [i32::MIN; 3];
const SAVE_SECTION: [i32; 3] = [i32::MIN, i32::MIN, i32::MIN + 1];
const PLACEMENT_SECTION: [i32; 3] = [i32::MIN, i32::MIN, i32::MIN + 2];

/// Drops everything the overlay shows (it is built again on the next tick).
pub fn clear() {
    mesh(|m| *m = Mesh::default());
    clear_scene();
}

/// The overlay's size: quads, sections built, sections still to build.
pub fn stats() -> (usize, usize, usize) {
    mesh(|m| (m.quads, m.built.len(), m.queue.len()))
}

/// Video memory a quad takes (4 vertices).
pub const QUAD_BYTES: usize = 4 * std::mem::size_of::<Vertex>();

/// Once per client tick, from the game side.
pub fn tick(host: &dyn ClientHost, ops: &dyn GameOps) {
    let w = World::new(host);
    let active = w.in_world() && runtime::with(|s| s.settings_ready());
    if !active {
        if mesh(|m| m.key.is_some() || !m.built.is_empty()) {
            clear();
        }
        options(false, false);
        return;
    }
    runtime::with(|s| {
        let any = s.slots.iter().any(|x| x.shown) || s.area.iter().any(Option::is_some) || s.save.is_some();
        options(s.settings.through, s.settings.render && any);
    });
    rebuild(&w, ops);
    marks_step(&w, ops);
}

// ---------------------------------------------------------------- building

/// Keeps the overlay up to date, one section at a time: a section is built again only when its
/// world section changed (a block set, a chunk loaded) or what it shows did, nearest first, and
/// only as many per tick as fit in `MESH_BUDGET`.
fn rebuild(w: &World<'_>, ops: &dyn GameOps) {
    let Some(me) = w.feet() else { return };
    let start = Instant::now();
    if take_lost() {
        mesh(|m| m.key = None);
    }
    let Some((key, anchor, boxes, radius, limit)) = runtime::with(|s| {
        let st = &s.settings;
        let key: MeshKey = (s.rev, s.layer, st.mode, st.show_extra, st.show_wrong, st.strict, st.power, st.alpha.to_bits(), st.hide_fluids);
        let boxes: Vec<_> = s.slots.iter().filter(|x| x.shown).map(|x| x.placement.bounds()).collect();
        // Positions are relative to the placements' corner, else (only an area) the area's.
        let anchor = s
            .shown_bounds()
            .or_else(|| s.area_box())
            .or_else(|| s.area.iter().flatten().next().map(|&c| (c, c)))
            .or_else(|| s.save.as_ref().map(|j| (j.lo, j.hi)))?
            .0;
        let limit = if st.max_faces == 0 { usize::MAX } else { st.max_faces };
        Some((key, anchor, boxes, runtime::render_radius(w, st), limit))
    }) else {
        if mesh(|m| m.key.is_some() || !m.built.is_empty()) {
            clear();
        }
        return;
    };
    if mesh(|m| m.key != Some(key) || m.anchor != anchor) {
        mesh(|m| {
            let cubes = std::mem::take(&mut m.cubes);
            *m = Mesh { key: Some(key), anchor, cubes, ..Mesh::default() };
        });
        clear_scene();
        set_anchor(anchor);
    }
    area_step(w, ops);
    save_frames_step(ops);
    placement_frames_step(ops);
    let full = mesh(|m| {
        m.tick += 1;
        m.tick % FAR_EVERY == 1
    }) || radius <= NEAR;
    let reach = if full { radius } else { NEAR };
    let mut wanted: HashSet<[i32; 3]> = HashSet::new();
    for (lo, hi) in boxes {
        let a = [0, 1, 2].map(|i| lo[i].max(me[i] - reach) >> 4);
        let b = [0, 1, 2].map(|i| hi[i].min(me[i] + reach) >> 4);
        for sy in a[1]..=b[1] {
            for sz in a[2]..=b[2] {
                for sx in a[0]..=b[0] {
                    wanted.insert([sx, sy, sz]);
                }
            }
        }
    }
    // Out of range: dropped (known only when the whole radius was gone through).
    if full {
        let gone: Vec<[i32; 3]> = mesh(|m| m.built.keys().filter(|k| !wanted.contains(*k)).copied().collect());
        for k in gone {
            mesh(|m| {
                if let Some((_, n)) = m.built.remove(&k) {
                    m.quads -= n;
                }
            });
            remove_section(k);
        }
    }
    // New or changed: built again, nearest first.
    let mut todo = Vec::new();
    mesh(|m| {
        for &k in &wanted {
            let now = ops.section(k);
            if m.built.get(&k).is_none_or(|(snap, _)| !same(snap, &now)) {
                let d = [0, 1, 2].map(|i| i64::from(k[i] * 16 + 8 - me[i]));
                todo.push((d[0] * d[0] + d[1] * d[1] + d[2] * d[2], k, now));
            }
        }
    });
    todo.sort_by_key(|t| (t.0, t.1));
    let mut queue = if full { VecDeque::new() } else { mesh(|m| std::mem::take(&mut m.queue)) };
    let mut todo = todo.into_iter().map(|(_, k, snap)| (k, Some(snap)));
    let mut stop = false;
    loop {
        let (k, snap) = match todo.next() {
            Some(t) => t,
            None if stop => break,
            None => match queue.pop_front() {
                Some(k) => (k, None),
                None => break,
            },
        };
        if stop {
            if full {
                queue.push_back(k);
            }
            continue;
        }
        let snap = match snap {
            Some(snap) => snap,
            None => {
                let now = ops.section(k);
                if mesh(|m| m.built.get(&k).is_some_and(|(snap, _)| same(snap, &now))) {
                    continue;
                }
                now
            }
        };
        if mesh(|m| m.quads >= limit && !m.built.contains_key(&k)) {
            runtime::with(|s| {
                s.printer_note = format!("overlay full ({}): raise the face limit or lower the render radius", crate::settings::faces_text(limit));
            });
            stop = true;
            if full {
                queue.push_back(k);
            }
            continue;
        }
        let quads = build_section(w, ops, k);
        let n = quads.len() / 4;
        mesh(|m| {
            let old = m.built.insert(k, (snap, n)).map_or(0, |(_, n)| n);
            m.quads = m.quads + n - old;
        });
        put_section(k, quads);
        stop = start.elapsed() > MESH_BUDGET;
    }
    mesh(|m| m.queue = queue);
}

/// Whether `state` is a full opaque cube, cached.
fn solid_cube(ops: &dyn GameOps, state: StateId) -> bool {
    if let Some(c) = mesh(|m| m.cubes.get(&state).copied()) {
        return c;
    }
    let c = ops.solid_cube(state);
    mesh(|m| m.cubes.insert(state, c));
    c
}

/// The overlay's quads for section `k`: ghosts of missing blocks (the faces between two ghost
/// cubes left out), boxes for the rest, each quad with its outward normal.
fn build_section(w: &World<'_>, ops: &dyn GameOps, k: [i32; 3]) -> Vec<Vertex> {
    let mut quads = Vec::new();
    let white = ops.white_uv();
    let anchor = mesh(|m| m.anchor);
    runtime::with(|s| {
        let Some((lo, hi)) = s.shown_bounds() else { return };
        let ghost_alpha = (s.settings.alpha.clamp(0.05, 1.0) * 255.0) as u32;
        let (mode, show_extra, show_wrong) = (s.settings.mode, s.settings.show_extra, s.settings.show_wrong);
        let hide_fluids = s.settings.hide_fluids;
        let layer = s.layer;
        let a = [0, 1, 2].map(|i| (k[i] * 16).max(lo[i]));
        let b = [0, 1, 2].map(|i| (k[i] * 16 + 15).min(hi[i]));
        // What each cell of the section and the layer of cells around it shows, as far as it
        // hides a neighbour's face: `NOTHING`, `CUBE` (a ghost full cube) or 2/3 (a ghost
        // water/lava). Filled in while building, the border on demand; `UNKNOWN` until then.
        const UNKNOWN: u8 = 255;
        const NOTHING: u8 = 0;
        const CUBE: u8 = 1;
        let base = [0, 1, 2].map(|i| k[i] * 16 - 1);
        let cell = |p: [i32; 3]| -> Option<usize> {
            let l = [0, 1, 2].map(|i| p[i] - base[i]);
            l.iter().all(|v| (0..18).contains(v)).then(|| ((l[1] * 18 + l[2]) * 18 + l[0]) as usize)
        };
        let mut shows = vec![UNKNOWN; 18 * 18 * 18];
        let class = |status: Status, res: &Resolved| -> u8 {
            match res.id {
                Some(id) if mode == Mode::Ghost && status == Status::Missing => {
                    if solid_cube(ops, id) {
                        CUBE
                    } else if res.fluid != 0 && !hide_fluids {
                        1 + res.fluid
                    } else {
                        NOTHING
                    }
                }
                _ => NOTHING,
            }
        };
        // Per quad: the block it belongs to, and what that block shows.
        let mut blocks: Vec<([i32; 3], u8)> = Vec::new();
        for y in a[1]..=b[1] {
            if !layer.contains(y) {
                continue;
            }
            for z in a[2]..=b[2] {
                for x in a[0]..=b[0] {
                    let p = [x, y, z];
                    let Some((status, _, res)) = runtime::quick_status(w, s, p) else {
                        if let Some(c) = cell(p) {
                            shows[c] = NOTHING;
                        }
                        continue;
                    };
                    let shown = class(status, &res);
                    if let Some(c) = cell(p) {
                        shows[c] = shown;
                    }
                    if hide_fluids && res.fluid != 0 {
                        continue;
                    }
                    let rel = [0, 1, 2].map(|i| (p[i] - anchor[i]) as f32);
                    let inflated = |e: f32| (rel.map(|v| v - e), rel.map(|v| v + 1.0 + e));
                    let mut cube = false;
                    match status {
                        Status::Correct => {}
                        Status::Missing => {
                            let start = quads.len();
                            if mode == Mode::Ghost
                                && let Some(id) = res.id
                            {
                                // Each face comes with its own normal (a repeater's top lies below
                                // the block's middle and still faces up); the directional shade the
                                // game would bake in is added here.
                                ops.block_model(id, rel, FULL_BRIGHT, &mut quads);
                                for v in &mut quads[start..] {
                                    v.color = shade(v.color, v.normal);
                                }
                                ops.special_model(id, rel, &mut quads);
                                for v in &mut quads[start..] {
                                    v.color = (v.color & 0x00FF_FFFF) | ghost_alpha << 24;
                                }
                                cube = shown != NOTHING;
                            }
                            if quads.len() == start {
                                let (lo, hi) = inflated(-0.2);
                                push_box(&mut quads, white, lo, hi, Status::Missing.overlay_argb() | 0x6000_0000);
                            }
                        }
                        Status::Extra if show_extra => {
                            let (lo, hi) = inflated(0.004);
                            push_box(&mut quads, white, lo, hi, Status::Extra.overlay_argb());
                        }
                        Status::WrongBlock | Status::WrongState if show_wrong => {
                            let (lo, hi) = inflated(0.004);
                            push_box(&mut quads, white, lo, hi, status.overlay_argb());
                        }
                        _ => {}
                    }
                    while blocks.len() < quads.len() / 4 {
                        blocks.push((p, if cube { shown } else { NOTHING }));
                    }
                }
            }
        }
        quads.truncate(blocks.len() * 4);
        let mut shows_at = |p: [i32; 3]| -> u8 {
            let Some(c) = cell(p) else { return NOTHING };
            if shows[c] == UNKNOWN {
                shows[c] = if layer.contains(p[1]) {
                    runtime::quick_status(w, s, p).map_or(NOTHING, |(st, _, res)| class(st, &res))
                } else {
                    NOTHING
                };
            }
            shows[c]
        };
        let mut kept = Vec::with_capacity(quads.len());
        for (q, &(p, own)) in quads.chunks_exact(4).zip(&blocks) {
            let block = [0, 1, 2].map(|i| (p[i] - anchor[i]) as f32 + 0.5);
            let c = [0, 1, 2].map(|i| (q[0].pos[i] + q[1].pos[i] + q[2].pos[i] + q[3].pos[i]) * 0.25);
            let out = sub(c, block);
            let given = q[0].normal;
            let geometric = normalize(cross(sub(q[1].pos, q[0].pos), sub(q[2].pos, q[0].pos)));
            // The model's own normal where it gives one (two-sided faces, w = -127, never hide);
            // for the boxes, away from the block's centre (their faces are its sides). A face
            // through the centre (a plant's cross) gets none and is always drawn.
            let normal = if given[3] == -127 {
                [0.0; 3]
            } else if given[..3] != [0, 0, 0] {
                normalize([f32::from(given[0]), f32::from(given[1]), f32::from(given[2])])
            } else if dot(out, out) < 1e-4 || dot(geometric, out).abs() < 1e-3 {
                [0.0; 3]
            } else if dot(geometric, out) < 0.0 {
                geometric.map(|v| -v)
            } else {
                geometric
            };
            let zero = normal == [0.0; 3];
            // A ghost cube's face on its boundary, against another ghost cube: hidden; a ghost
            // fluid's against a cube or the same fluid too.
            if own != NOTHING
                && !zero
                && let Some(i) = (0..3).find(|&i| normal[i].abs() > 0.99)
                && (out[i].abs() - 0.5).abs() < 1e-3
            {
                let mut n = p;
                n[i] += normal[i].signum() as i32;
                let next = shows_at(n);
                if next == CUBE || (own != CUBE && next == own) {
                    continue;
                }
            }
            let packed = if zero {
                [0; 4]
            } else {
                let n = normal.map(|v| (v * 127.0).round() as i8);
                [n[0], n[1], n[2], 127]
            };
            for v in q {
                let mut v = *v;
                v.normal = packed;
                kept.push(v);
            }
        }
        quads = kept;
    });
    quads
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn normalize(a: [f32; 3]) -> [f32; 3] {
    let l = dot(a, a).sqrt();
    if l > 1e-12 { a.map(|v| v / l) } else { [0.0; 3] }
}

// ---------------------------------------------------------------- highlights

/// Keeps the highlights up to date for the shown placements, however far they reach (the render
/// radius and the face limit don't count here), nearest first.
fn marks_step(w: &World<'_>, ops: &dyn GameOps) {
    let Some(me) = w.feet() else { return };
    if mesh(|m| m.key.is_none()) {
        return;
    }
    let start = Instant::now();
    let (key, boxes) = runtime::with(|s| {
        let key = (s.settings.highlight_missing, s.settings.highlight_wrong, s.highlight_item.clone());
        let boxes: Vec<_> = s.slots.iter().filter(|x| x.shown).map(|x| x.placement.bounds()).collect();
        (key, boxes)
    });
    let on = key.0 || key.1;
    let changed = mesh(|m| {
        let changed = m.marks_key.as_ref() != Some(&key);
        if changed {
            m.marks_key = Some(key);
            m.marks.clear();
            m.marks_count = 0;
            m.marks_queue.clear();
        }
        changed
    });
    if changed {
        clear_marks();
    }
    if !on {
        return;
    }
    let full = changed || mesh(|m| m.tick % FAR_EVERY == 1);
    let reach = if full { i32::MAX / 2 } else { NEAR };
    let mut wanted: HashSet<[i32; 3]> = HashSet::new();
    for (lo, hi) in boxes {
        let a = [0, 1, 2].map(|i| lo[i].max(me[i].saturating_sub(reach)) >> 4);
        let b = [0, 1, 2].map(|i| hi[i].min(me[i].saturating_add(reach)) >> 4);
        for sy in a[1]..=b[1] {
            for sz in a[2]..=b[2] {
                for sx in a[0]..=b[0] {
                    wanted.insert([sx, sy, sz]);
                }
            }
        }
    }
    let dist = |k: [i32; 3]| {
        let d = [0, 1, 2].map(|i| i64::from(k[i]) * 16 + 8 - i64::from(me[i]));
        d[0] * d[0] + d[1] * d[1] + d[2] * d[2]
    };
    if full {
        let gone: Vec<[i32; 3]> = mesh(|m| {
            let gone: Vec<_> = m.marks.keys().filter(|k| !wanted.contains(*k)).copied().collect();
            for k in &gone {
                if let Some((_, n)) = m.marks.remove(k) {
                    m.marks_count -= n;
                }
            }
            m.marks_queue.clear();
            gone
        });
        for k in gone {
            put_marks(k, Vec::new());
        }
    }
    let mut todo: Vec<[i32; 3]> =
        mesh(|m| wanted.iter().filter(|&&k| m.marks.get(&k).is_none_or(|(snap, _)| !same(snap, &ops.section(k)))).copied().collect());
    todo.sort_by_key(|&k| (dist(k), k));
    let mut queue: VecDeque<[i32; 3]> = todo.into();
    queue.extend(mesh(|m| std::mem::take(&mut m.marks_queue)));
    while let Some(k) = queue.pop_front() {
        let snap = ops.section(k);
        if mesh(|m| m.marks.get(&k).is_some_and(|(old, _)| same(old, &snap))) {
            continue;
        }
        let (quads, n) = build_marks(w, ops, k);
        // Over the limit: the farthest make room for nearer ones; if none is farther, this is as
        // far as it goes, until the player moves.
        let (fits, dropped) = mesh(|m| {
            let mut dropped = Vec::new();
            let old = m.marks.get(&k).map_or(0, |(_, n)| *n);
            while m.marks_count - old + n > MAX_MARKS {
                let far = m.marks.iter().filter(|(q, (_, c))| **q != k && *c > 0).map(|(q, _)| *q).max_by_key(|&q| dist(q));
                let Some(far) = far.filter(|&q| dist(q) > dist(k)) else {
                    return (false, dropped);
                };
                if let Some((_, c)) = m.marks.remove(&far) {
                    m.marks_count -= c;
                }
                dropped.push(far);
            }
            m.marks_count = m.marks_count - old + n;
            m.marks.insert(k, (snap, n));
            (true, dropped)
        });
        for far in dropped {
            put_marks(far, Vec::new());
        }
        if !fits {
            queue.clear();
            break;
        }
        put_marks(k, quads);
        if start.elapsed() > MARKS_BUDGET {
            break;
        }
    }
    mesh(|m| m.marks_queue = queue);
}

/// The highlights of section `k`: a box over each block to mark, seen through everything, and how
/// many there are. Missing and wrong go by the item wanted there, as the material list counts.
fn build_marks(w: &World<'_>, ops: &dyn GameOps, k: [i32; 3]) -> (Vec<Vertex>, usize) {
    let mut quads = Vec::new();
    let white = ops.white_uv();
    let anchor = mesh(|m| m.anchor);
    let mut count = 0;
    runtime::with(|s| {
        let Some((lo, hi)) = s.shown_bounds() else { return };
        let (missing, wrong) = (s.settings.highlight_missing, s.settings.highlight_wrong);
        let hide_fluids = s.settings.hide_fluids;
        let only = s.highlight_item.clone();
        let layer = s.layer;
        let a = [0, 1, 2].map(|i| (k[i] * 16).max(lo[i]));
        let b = [0, 1, 2].map(|i| (k[i] * 16 + 15).min(hi[i]));
        for y in a[1]..=b[1] {
            if !layer.contains(y) {
                continue;
            }
            for z in a[2]..=b[2] {
                for x in a[0]..=b[0] {
                    let p = [x, y, z];
                    let Some((status, at, res)) = runtime::quick_status(w, s, p) else { continue };
                    let argb = match status {
                        Status::Missing if missing => 0x8033_B3E6u32,
                        Status::WrongBlock if wrong => 0x80FF_3333,
                        Status::WrongState if wrong => 0x80FF_9010,
                        _ => continue,
                    };
                    if hide_fluids && res.fluid != 0 {
                        continue;
                    }
                    if let Some(only) = &only {
                        let wanted = s.state_of(at).clone();
                        if runtime::item_for(w, s, &wanted).as_ref() != Some(only) {
                            continue;
                        }
                    }
                    let lo = [0, 1, 2].map(|i| (p[i] - anchor[i]) as f32 - 0.01);
                    let start = quads.len();
                    push_box(&mut quads, white, lo, lo.map(|v| v + 1.02), argb);
                    // Outward normals in `push_box`'s order: only the near faces are drawn, so a
                    // block's far side doesn't darken it.
                    const OUT: [[i8; 4]; 6] =
                        [[0, 127, 0, 127], [0, -127, 0, 127], [0, 0, -127, 127], [0, 0, 127, 127], [-127, 0, 0, 127], [127, 0, 0, 127]];
                    for (face, q) in quads[start..].chunks_exact_mut(4).enumerate() {
                        for v in q {
                            v.normal = OUT[face];
                        }
                    }
                    count += 1;
                }
            }
        }
    });
    (quads, count)
}

// ---------------------------------------------------------------- frames

/// The area selection's frame when it changed: corner 1 in red, corner 2 in blue (as Litematica
/// marks them), the box's edges white (yellow when the tool has the whole box picked).
fn area_step(w: &World<'_>, ops: &dyn GameOps) {
    let tool = runtime::tool_held(w);
    let anchor = mesh(|m| m.anchor);
    let now = runtime::with(|s| (s.area, anchor, tool.then_some(s.grab)));
    if mesh(|m| m.area == Some(now)) {
        return;
    }
    mesh(|m| m.area = Some(now));
    let (area, anchor, grab) = now;
    let mut quads = Vec::new();
    let white = ops.white_uv();
    let rel = |p: [i32; 3]| [0, 1, 2].map(|i| (p[i] - anchor[i]) as f32);
    for (i, (corner, argb)) in [(area[0], 0x80FF_3030u32), (area[1], 0x8030_60FF)].into_iter().enumerate() {
        if let Some(c) = corner {
            let r = rel(c);
            let (argb, out) = if grab == Some(Grab::Corner(i)) { (argb | 0xD000_0000, 0.04) } else { (argb, 0.01) };
            push_box(&mut quads, white, r.map(|v| v - out), r.map(|v| v + 1.0 + out), argb);
        }
    }
    let edge = if grab == Some(Grab::Box) { 0xE0FF_FF55 } else { 0xE0FF_FFFF };
    if let [Some(a), Some(b)] = area {
        let lo = rel([0, 1, 2].map(|i| a[i].min(b[i])));
        let hi = rel([0, 1, 2].map(|i| a[i].max(b[i]) + 1));
        push_frame(&mut quads, white, lo, hi, 0.03, edge);
    }
    if quads.is_empty() {
        remove_section(AREA_SECTION);
    } else {
        put_section(AREA_SECTION, quads);
    }
}

/// The frames of the chunks a save still waits for.
fn save_frames_step(ops: &dyn GameOps) {
    let anchor = mesh(|m| m.anchor);
    let (chunks, lo, hi) = runtime::with(|s| match &s.save {
        Some(job) => (job.unloaded.iter().take(SAVE_FRAMES).copied().collect::<Vec<_>>(), job.lo, job.hi),
        None => (Vec::new(), [0; 3], [0; 3]),
    });
    let now = (chunks, anchor);
    if mesh(|m| m.save.as_ref() == Some(&now)) {
        return;
    }
    mesh(|m| m.save = Some(now.clone()));
    let (chunks, _) = now;
    if chunks.is_empty() {
        remove_section(SAVE_SECTION);
        return;
    }
    let white = ops.white_uv();
    let rel = |p: [i32; 3]| [0, 1, 2].map(|i| (p[i] - anchor[i]) as f32);
    let mut quads = Vec::new();
    for (cx, cz) in chunks {
        let a = [(cx * 16).max(lo[0]), lo[1], (cz * 16).max(lo[2])];
        let b = [(cx * 16 + 15).min(hi[0]) + 1, hi[1] + 1, (cz * 16 + 15).min(hi[2]) + 1];
        push_frame(&mut quads, white, rel(a), rel(b), 0.08, 0xE0FF_8800);
    }
    put_section(SAVE_SECTION, quads);
}

/// The frames around the placements: the selected one bright (cyan, or orange when locked) and
/// thicker, the other shown ones faint.
fn placement_frames_step(ops: &dyn GameOps) {
    let anchor = mesh(|m| m.anchor);
    let frames: Vec<_> = runtime::with(|s| {
        if !s.settings.placement_boxes {
            return Vec::new();
        }
        s.slots
            .iter()
            .enumerate()
            .filter(|(i, x)| x.shown || *i == s.selected)
            .map(|(i, x)| {
                let (lo, hi) = x.placement.bounds();
                let (argb, thick) = match (i == s.selected, x.locked) {
                    (true, false) => (0xE055_FFFF, 4),
                    (true, true) => (0xE0FF_AA00, 4),
                    (false, _) => (0x70C0_C0C0, 2),
                };
                (lo, hi, argb, thick)
            })
            .collect()
    });
    let now = (frames, anchor);
    if mesh(|m| m.frames.as_ref() == Some(&now)) {
        return;
    }
    mesh(|m| m.frames = Some(now.clone()));
    let white = ops.white_uv();
    let rel = |p: [i32; 3]| [0, 1, 2].map(|i| (p[i] - anchor[i]) as f32);
    let mut quads = Vec::new();
    for (lo, hi, argb, thick) in now.0 {
        push_frame(&mut quads, white, rel(lo), rel(hi.map(|v| v + 1)), thick as f32 / 100.0, argb);
    }
    if quads.is_empty() {
        remove_section(PLACEMENT_SECTION);
    } else {
        put_section(PLACEMENT_SECTION, quads);
    }
}

// ---------------------------------------------------------------- quads

fn argb_to_vertex(argb: u32, shade: f32) -> u32 {
    let [a, r, g, b] = argb.to_be_bytes();
    let f = |c: u8| (f32::from(c) * shade).round().clamp(0.0, 255.0) as u32;
    f(r) | f(g) << 8 | f(b) << 16 | u32::from(a) << 24
}

/// The 12 edges of the box `lo..hi` as thin boxes `t` thick: along each axis, at the four
/// combinations of the other two.
fn push_frame(out: &mut Vec<Vertex>, white: [f32; 2], lo: [f32; 3], hi: [f32; 3], t: f32, argb: u32) {
    for axis in 0..3 {
        let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
        for cu in [lo[u], hi[u]] {
            for cv in [lo[v], hi[v]] {
                let mut min = [0.0; 3];
                let mut max = [0.0; 3];
                min[axis] = lo[axis] - t;
                max[axis] = hi[axis] + t;
                min[u] = cu - t;
                max[u] = cu + t;
                min[v] = cv - t;
                max[v] = cv + t;
                push_box(out, white, min, max, argb);
            }
        }
    }
}

/// A box's six faces as quads, on the atlas's white texel: up, down, north, south, west, east.
fn push_box(out: &mut Vec<Vertex>, white: [f32; 2], min: [f32; 3], max: [f32; 3], argb: u32) {
    let [x0, y0, z0] = min;
    let [x1, y1, z1] = max;
    let faces: [([[f32; 3]; 4], f32); 6] = [
        ([[x0, y1, z0], [x0, y1, z1], [x1, y1, z1], [x1, y1, z0]], 1.0),
        ([[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]], 0.5),
        ([[x0, y0, z0], [x0, y1, z0], [x1, y1, z0], [x1, y0, z0]], 0.8),
        ([[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]], 0.8),
        ([[x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0]], 0.6),
        ([[x1, y0, z0], [x1, y1, z0], [x1, y1, z1], [x1, y0, z1]], 0.6),
    ];
    for (corners, shade) in faces {
        let color = argb_to_vertex(argb, shade);
        for pos in corners {
            out.push(Vertex { pos, uv: white, layer: 0, color, light: FULL_BRIGHT, normal: [0; 4], glint_uv: [0.0, 0.0] });
        }
    }
}

/// A block face's colour with vanilla's directional shade (`BlockModelShaper`: up 1, down 0.5,
/// north/south 0.8, east/west 0.6), from its normal.
fn shade(color: u32, normal: [i8; 4]) -> u32 {
    let [nx, ny, nz, _] = normal.map(i32::from);
    let shade = if ny > 64 {
        255
    } else if ny < -64 {
        127
    } else if nz.abs() >= nx.abs() {
        204
    } else {
        153
    };
    let [r, g, b, a] = color.to_le_bytes();
    let f = |c: u8| (u32::from(c) * shade / 255) as u8;
    u32::from_le_bytes([f(r), f(g), f(b), a])
}
