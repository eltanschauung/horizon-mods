//! Horizonmatica's state and everything it does each tick, on commands, keys and clicks. The game
//! is read through `World` (the mod API); the game-independent logic is `horizonmatica_core`.

use crate::settings::{self, Mode, Settings};
use crate::store;
use crate::world::{Blocks, StateId, World};
use crate::CREDITS;
use horizonmatica_core::formats::Format;
use horizonmatica_core::materials::{self, Material};
use horizonmatica_core::printer::{Recent, Stuck};
use horizonmatica_core::verify::{self, Status, Tally};
use horizonmatica_core::{BlockState, LayerMode, Mirror, Placement, Rotation, Schematic};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};

/// Cells the verifier scans per tick.
const SCAN_PER_TICK: u64 = 32_768;

// ---------------------------------------------------------------- state

#[derive(Default)]
struct Scan {
    /// The placement scanned: its slot id and revision.
    key: (u64, u64),
    index: u64,
    tally: Tally,
    missing: BTreeMap<String, u64>,
    unloaded: u64,
}

/// One loaded schematic. Several can be loaded at once; one of them is selected, and the
/// commands, move mode, the verifier's HUD and the material list are about that one.
pub struct Slot {
    /// Unique for the session (never reused): caches key palette entries by it.
    pub id: u64,
    pub placement: Placement,
    /// Drawn and printed. A hidden placement stays loaded and remembered.
    pub shown: bool,
    /// Can't be moved, turned or mirrored (by commands, move mode or the tool).
    pub locked: bool,
    /// Every item the placement needs (counted once on load).
    pub totals: BTreeMap<String, u64>,
}

/// Where a block of a placement comes from: (slot index, (region, palette index)).
pub type Ref = (usize, (usize, u32));

/// What the game makes of a palette entry (cached per placement revision).
#[derive(Clone, Copy, Debug)]
pub struct Resolved {
    pub id: Option<StateId>,
    pub air: bool,
    /// Water (1) or lava (2) itself, not a waterlogged block; 0 for anything else.
    pub fluid: u8,
}

/// What the selection tool's wheel moves (Litematica's selected corner or box).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Grab {
    /// The whole box: both corners.
    #[default]
    Box,
    /// One corner (0 or 1).
    Corner(usize),
}

#[derive(Default)]
pub struct State {
    settings_loaded: bool,
    /// Game ticks seen (client ticks while in a world).
    pub ticks: u64,
    /// The world (and dimension) the placements belong to: `store::world_key`.
    world_key: Option<String>,
    pub settings: Settings,
    /// Lines for the chat, sent at the next hook that may write to it.
    pub out: Vec<String>,
    /// The loaded placements, drawn in this order (a later one's blocks win where they overlap).
    pub slots: Vec<Slot>,
    /// Index into `slots` of the selected placement.
    pub selected: usize,
    next_id: u64,
    /// Bumped whenever a placement is added, removed, shown, hidden, moved or turned
    /// (`sync_rev`): everything built from them is stale.
    pub rev: u64,
    rev_of: Vec<(u64, u64, bool)>,
    /// The area selection's two corners (`/hm pos1`, `/hm pos2`), saved with `/hm save`.
    pub area: [Option<[i32; 3]>; 2],
    pub layer: LayerMode,
    pub blocks: Blocks,
    /// Item per wanted state (None: no item, such as air or fire).
    items: HashMap<BlockState, Option<String>>,
    unknown: Vec<String>,
    scan: Scan,
    /// The verifier's last full result, for the slot id `tally_of`.
    tally_of: u64,
    tally: Option<Tally>,
    pub missing: BTreeMap<String, u64>,
    unloaded: u64,
    /// Palette entry (slot id, region, index) -> what the game makes of it, for `resolved_rev`.
    resolved: HashMap<(u64, usize, u32), Resolved>,
    resolved_rev: u64,
    pub recent: Recent,
    pub stuck: Stuck,
    /// Move mode: the wheel and the arrow keys move the placement.
    pub moving: bool,
    /// Wheel steps not yet a whole block (touchpads send fractions).
    wheel: f32,
    /// Schematic files being read (`start_load`).
    loads: Vec<LoadJob>,
    /// The save under way (`/hm save`), and the file its thread is writing.
    pub save: Option<SaveJob>,
    save_write: Option<SaveWrite>,
    /// What the selection tool's wheel moves, and the mouse buttons whose press it took.
    pub grab: Grab,
    tool_down: [bool; 3],
    /// The credits line was shown this session (on the first load).
    credited: bool,
    pub printer_note: String,
    pub easy_note: String,
    pub placed: u64,
    /// The highlights show only blocks of this item (a session's choice).
    pub highlight_item: Option<String>,
    /// The last message a command or a load gave (the menu shows it).
    pub message: String,
}

impl State {
    /// The settings were read from the store (once per session, in a world).
    pub fn settings_ready(&self) -> bool {
        self.settings_loaded
    }
    pub fn sel(&self) -> Option<&Slot> {
        self.slots.get(self.selected)
    }
    pub fn placement(&self) -> Option<&Placement> {
        self.sel().map(|s| &s.placement)
    }
    fn placement_mut(&mut self) -> Option<&mut Placement> {
        self.slots.get_mut(self.selected).map(|s| &mut s.placement)
    }
    /// The verifier's last full result, when it is the selected placement's.
    pub fn tally(&self) -> Option<&Tally> {
        self.tally.as_ref().filter(|_| self.sel().is_some_and(|x| x.id == self.tally_of))
    }
    pub fn state_of(&self, (slot, r): Ref) -> &BlockState {
        self.slots[slot].placement.palette_state(r)
    }
    /// Bumps `rev` when the placements changed since the last call.
    fn sync_rev(&mut self) {
        let now: Vec<(u64, u64, bool)> = self.slots.iter().map(|s| (s.id, s.placement.revision, s.shown)).collect();
        if now != self.rev_of {
            self.rev_of = now;
            self.rev += 1;
        }
    }
    /// The block wanted at `w`: the last shown placement's with one there. As with regions, a
    /// later placement's air doesn't erase an earlier placement's block.
    pub fn wanted_at(&self, w: [i32; 3]) -> Option<Ref> {
        let mut found: Option<Ref> = None;
        for (i, slot) in self.slots.iter().enumerate() {
            if !slot.shown {
                continue;
            }
            if let Some(r) = slot.placement.wanted_ref(w)
                && (found.is_none() || !slot.placement.palette_state(r).is_air())
            {
                found = Some((i, r));
            }
        }
        found
    }
    /// The box around every shown placement (min, max inclusive).
    pub fn shown_bounds(&self) -> Option<([i32; 3], [i32; 3])> {
        self.slots.iter().filter(|s| s.shown).map(|s| s.placement.bounds()).reduce(|(alo, ahi), (blo, bhi)| {
            ([0, 1, 2].map(|i| alo[i].min(blo[i])), [0, 1, 2].map(|i| ahi[i].max(bhi[i])))
        })
    }
    /// The area selection's box, once both corners are set.
    pub fn area_box(&self) -> Option<([i32; 3], [i32; 3])> {
        let [Some(a), Some(b)] = self.area else { return None };
        Some(([0, 1, 2].map(|i| a[i].min(b[i])), [0, 1, 2].map(|i| a[i].max(b[i]))))
    }
    /// The area selection's size in blocks, checked (`None` without both corners).
    pub fn area_size(&self) -> Option<[i32; 3]> {
        let [Some(a), Some(b)] = self.area else { return None };
        horizonmatica_core::area::size(a, b)
    }
    /// Drops every placement (another world, `/hm unload all`).
    fn drop_all(&mut self) {
        reset_caches(self);
        self.slots.clear();
        self.selected = 0;
        self.settings.printer_on = false;
        self.moving = false;
    }
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

pub fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with_borrow_mut(f)
}

/// Queues a chat line (`flush` sends it).
pub fn say(text: &str) {
    with(|s| s.out.push(format!("[Horizonmatica] {text}")));
}

/// The chat lines queued, to send.
pub fn take_out() -> Vec<String> {
    with(|s| std::mem::take(&mut s.out))
}

fn report(r: Result<Vec<String>, String>) {
    match r {
        Ok(lines) => {
            for l in lines {
                say(&l);
            }
        }
        Err(e) => say(&format!("Error: {e}")),
    }
}

// ---------------------------------------------------------------- settings and placements

pub fn ensure_settings(w: &World<'_>) {
    if with(|s| s.settings_loaded) {
        return;
    }
    settings::migrate(w);
    with(|s| {
        s.settings = settings::load(w, s.settings.printer_on);
        s.settings_loaded = true;
    });
}

/// The settings changed elsewhere (the mod's page in Options > Mods): read again.
pub fn settings_changed() {
    with(|s| s.settings_loaded = false);
}

pub fn save_settings(w: &World<'_>) {
    let st = with(|s| s.settings.clone());
    settings::save(w, &st);
}

pub fn render_radius(w: &World<'_>, st: &Settings) -> i32 {
    if st.radius_view {
        w.host.graphics_options().render_distance.max(1) as i32 * 16
    } else {
        st.radius
    }
}

/// Where a saved placement goes: origin, rotation, mirror.
type At = ([i32; 3], Rotation, Mirror);

/// Saves where the placements are: the selected one as the last one (`/hm reload`), all of them
/// as this world's, which come back when the world is joined again.
///
/// A world's value is `v2`, the selected index, then one line per placement:
/// `x|y|z|rotation|mirror|flags|file` (flags: 1 shown, 2 locked).
pub fn save_placements(w: &World<'_>) {
    let (last, world) = with(|s| {
        let last = s.placement().map(|p| {
            format!("{}|{}|{}|{}|{}|{}", p.file_name, p.origin[0], p.origin[1], p.origin[2], p.rotation.name(), p.mirror.name())
        });
        let mut world = if s.slots.is_empty() { String::new() } else { format!("v2\n{}", s.selected) };
        for slot in &s.slots {
            let p = &slot.placement;
            world += &format!(
                "\n{}|{}|{}|{}|{}|{}|{}",
                p.origin[0],
                p.origin[1],
                p.origin[2],
                p.rotation.name(),
                p.mirror.name(),
                u8::from(slot.shown) | u8::from(slot.locked) << 1,
                p.file_name
            );
        }
        (last, world)
    });
    if let Some(last) = last {
        store::write_value(w, "last", &last);
    }
    if let Some(key) = store::world_key(w) {
        store::write_value(w, &key, &world);
    }
}

/// A world's saved placements (either format): the selected index, and per placement its file,
/// where it goes, and whether it is shown and locked.
fn parse_world(v: &str) -> (usize, Vec<(String, At, (bool, bool))>) {
    let mut lines = v.lines();
    if lines.next() != Some("v2") {
        return (0, parse_saved(v).map(|(file, at)| (file, at, (true, false))).into_iter().collect());
    }
    let selected = lines.next().and_then(|l| l.trim().parse().ok()).unwrap_or(0);
    let entries = lines
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(7, '|').collect();
            if parts.len() != 7 || parts[6].is_empty() {
                return None;
            }
            let n = |i: usize| parts[i].parse::<i32>().ok();
            let at = ([n(0)?, n(1)?, n(2)?], Rotation::parse(parts[3]).unwrap_or_default(), Mirror::parse(parts[4]).unwrap_or_default());
            let flags = parts[5].parse::<u8>().unwrap_or(1);
            Some((parts[6].to_string(), at, (flags & 1 != 0, flags & 2 != 0)))
        })
        .collect();
    (selected, entries)
}

/// A saved placement: file, origin, rotation, mirror.
fn parse_saved(v: &str) -> Option<(String, At)> {
    let parts: Vec<&str> = v.trim().split('|').collect();
    if parts.len() != 6 || parts[0].is_empty() {
        return None;
    }
    let n = |i: usize| parts[i].parse::<i32>().ok();
    let at = ([n(1)?, n(2)?, n(3)?], Rotation::parse(parts[4]).unwrap_or_default(), Mirror::parse(parts[5]).unwrap_or_default());
    Some((parts[0].to_string(), at))
}

/// Joining a world (or another dimension of it): its placements come back where they were left;
/// those of another world go, and so does the area selection.
fn restore_step(w: &World<'_>) {
    let Some(key) = store::world_key(w) else { return };
    if with(|s| s.world_key.as_deref() == Some(key.as_str())) {
        return;
    }
    with(|s| {
        s.world_key = Some(key.clone());
        s.drop_all();
        s.area = [None; 2];
    });
    crate::overlay::clear();
    let Some(saved) = store::read_value(w, &key) else { return };
    let (selected, entries) = parse_world(&saved);
    if entries.is_empty() {
        return;
    }
    let flags = entries.iter().map(|e| e.2).collect();
    let files = entries.into_iter().map(|(file, at, _)| (file, at)).collect();
    let lines = start_load(w, files, Then::Restore { flags, selected }).unwrap_or_else(|e| vec![format!("Couldn't restore: {e}")]);
    for l in lines {
        say(&l);
    }
}

// ---------------------------------------------------------------- block lookups

fn reset_caches(s: &mut State) {
    // Keyed by palette index and checked only against the placement's revision, which a new
    // placement starts again at 0: without this a new schematic shows the old one's blocks.
    s.resolved.clear();
    s.items.clear();
    s.unknown.clear();
    s.scan = Scan::default();
    s.tally = None;
    s.missing.clear();
    s.unloaded = 0;
    s.recent.clear();
    s.stuck.clear();
}

pub fn wanted_id(w: &World<'_>, s: &mut State, st: &BlockState) -> Option<StateId> {
    let id = s.blocks.resolve(w, st);
    if id.is_none() && !st.is_air() && !st.is_void() && !s.unknown.contains(&st.name) {
        s.unknown.push(st.name.clone());
    }
    id
}

pub fn item_for(w: &World<'_>, s: &mut State, st: &BlockState) -> Option<String> {
    if let Some(i) = s.items.get(st) {
        return i.clone();
    }
    let item = match st.name.as_str() {
        "minecraft:water" | "minecraft:bubble_column" => Some("minecraft:water_bucket".to_string()),
        "minecraft:lava" => Some("minecraft:lava_bucket".to_string()),
        name if materials::planted_by(name).is_some() => materials::planted_by(name).map(String::from),
        _ => wanted_id(w, s, st).and_then(|id| s.blocks.item(w, id)),
    };
    s.items.insert(st.clone(), item.clone());
    item
}

fn fluid_of(st: &BlockState) -> u8 {
    match st.name.as_str() {
        "minecraft:water" | "minecraft:bubble_column" => 1,
        "minecraft:lava" => 2,
        _ => 0,
    }
}

pub fn resolve_ref(w: &World<'_>, s: &mut State, at: Ref) -> Option<Resolved> {
    if s.resolved_rev != s.rev {
        s.resolved.clear();
        s.resolved_rev = s.rev;
    }
    let key = (s.slots.get(at.0)?.id, at.1.0, at.1.1);
    if let Some(v) = s.resolved.get(&key) {
        return Some(*v);
    }
    let state = s.state_of(at).clone();
    let id = wanted_id(w, s, &state);
    let res = Resolved { id, air: state.is_air(), fluid: fluid_of(&state) };
    s.resolved.insert(key, res);
    Some(res)
}

/// The verifier's verdict at `w` against the shown placements (None: outside them, unloaded, or
/// nothing to say), with where its wanted state is and what the game makes of it.
pub fn quick_status(w: &World<'_>, s: &mut State, p: [i32; 3]) -> Option<(Status, Ref, Resolved)> {
    let at = s.wanted_at(p)?;
    judge(w, s, p, at)
}

/// The verdict at `p` against the placement in slot `slot` alone (shown or not).
fn quick_status_in(w: &World<'_>, s: &mut State, slot: usize, p: [i32; 3]) -> Option<(Status, Ref, Resolved)> {
    let r = s.slots.get(slot)?.placement.wanted_ref(p)?;
    judge(w, s, p, (slot, r))
}

/// The verdict at `p` for the wanted block `at`. The common cases (the right block, air where a
/// block is missing) need no state compared.
fn judge(w: &World<'_>, s: &mut State, p: [i32; 3], at: Ref) -> Option<(Status, Ref, Resolved)> {
    let res = resolve_ref(w, s, at)?;
    let actual_id = w.block(p)?;
    if !res.air && res.id == Some(actual_id) {
        return Some((Status::Correct, at, res));
    }
    let actual = s.blocks.state(w, actual_id);
    if actual.is_air() {
        return (!res.air).then_some((Status::Missing, at, res));
    }
    let replaceable = s.blocks.replaceable(w, actual_id) || (fluid_of(&actual) != 0 && actual.prop("level") != Some("0"));
    let replaceable = replaceable || fluid_of(&actual) != 0;
    let wanted = s.state_of(at).clone();
    let status = verify::classify_power(Some(&wanted), &actual, replaceable, s.settings.strict, s.settings.power)?;
    Some((status, at, res))
}

/// The verifier's verdict at `p` against the shown placements, and the state wanted there.
pub fn status_at(w: &World<'_>, s: &mut State, p: [i32; 3]) -> Option<(Status, BlockState)> {
    let (status, at, _) = quick_status(w, s, p)?;
    Some((status, s.state_of(at).clone()))
}

// ---------------------------------------------------------------- loading

/// Counts every item a placement needs (once per load).
fn count_totals(w: &World<'_>, s: &mut State, p: &Placement, histograms: &[Vec<u64>]) -> BTreeMap<String, u64> {
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    for (ri, histogram) in histograms.iter().enumerate() {
        let palette = &p.palettes()[ri];
        for (i, &n) in histogram.iter().enumerate() {
            let st = &palette[i];
            let per = u64::from(materials::items_for_state(st));
            if n == 0 || per == 0 {
                continue;
            }
            if let Some(item) = item_for(w, s, st) {
                *counts.entry(item).or_insert(0) += n * per;
            }
        }
    }
    counts
}

/// What a placement is called: its schematic's name, else its file's.
pub fn display_name(p: &Placement) -> String {
    if p.schematic.metadata.name.is_empty() { p.file_name.clone() } else { p.schematic.metadata.name.clone() }
}

/// A schematic file as read off the game thread: parsed, and its cells counted per palette entry
/// (for the material totals). Placing it is quick.
struct ReadFile {
    file: String,
    schematic: Schematic,
    format: Format,
    /// The data version it was saved with, and how many block states were brought up to date.
    from_version: i64,
    upgraded: usize,
    histograms: Vec<Vec<u64>>,
}

fn read_file(dir: &std::path::Path, query: &str) -> Result<ReadFile, String> {
    let file = horizonmatica_core::find_schematic(dir, query)?;
    let bytes = std::fs::read(dir.join(&file)).map_err(|e| format!("{file}: {e}"))?;
    let (mut schematic, format) = horizonmatica_core::formats::load(&bytes).map_err(|e| format!("{file}: {e}"))?;
    drop(bytes);
    let from_version = schematic.data_version;
    let upgraded = crate::fix::upgrade(&mut schematic);
    let histograms = schematic.regions.iter().map(|r| r.histogram()).collect();
    Ok(ReadFile { file, schematic, format, from_version, upgraded, histograms })
}

/// What becomes of files once they are read.
enum Then {
    /// `/hm load`: a new placement, selected.
    Load,
    /// `/hm reload`: replaces the placement of that file at that place, else a new one.
    Reload,
    /// Joining a world: its placements back, each shown or hidden, and the one selected.
    Restore { flags: Vec<(bool, bool)>, selected: usize },
}

/// Files a thread of their own is reading (`start_load`); `load_step` places them.
struct LoadJob {
    files: Vec<(String, At)>,
    then: Then,
    /// The world they are for: left before they are in, they are dropped.
    world: Option<String>,
    started: std::time::Instant,
    bytes: u64,
    done: std::sync::mpsc::Receiver<Vec<Result<ReadFile, String>>>,
}

impl LoadJob {
    fn label(&self) -> String {
        match self.files.as_slice() {
            [(f, _)] => f.clone(),
            many => format!("{} placements", many.len()),
        }
    }
}

/// How long a command waits for its files before they go on loading in the background.
const LOAD_WAIT: std::time::Duration = std::time::Duration::from_millis(150);

fn start_load(w: &World<'_>, files: Vec<(String, At)>, then: Then) -> Result<Vec<String>, String> {
    use std::sync::mpsc::RecvTimeoutError;
    let dir = store::schematics_dir(w)?;
    let queries: Vec<String> = files.iter().map(|(f, _)| f.clone()).collect();
    let bytes = queries
        .iter()
        .filter_map(|q| horizonmatica_core::find_schematic(&dir, q).ok())
        .filter_map(|f| std::fs::metadata(dir.join(f)).ok())
        .map(|m| m.len())
        .sum();
    let (tx, done) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("horizonmatica-load".into())
        .spawn(move || {
            let _ = tx.send(queries.iter().map(|q| read_file(&dir, q)).collect());
        })
        .map_err(|e| format!("can't start reading: {e}"))?;
    let job = LoadJob { files, then, world: store::world_key(w), started: std::time::Instant::now(), bytes, done };
    match job.done.recv_timeout(LOAD_WAIT) {
        Ok(read) => finish_load(w, job, read),
        Err(RecvTimeoutError::Disconnected) => Err(format!("reading {} failed", job.label())),
        Err(RecvTimeoutError::Timeout) => {
            let line = format!("Loading {} ({}) in the background...", job.label(), human_size(job.bytes));
            with(|s| s.loads.push(job));
            Ok(vec![line])
        }
    }
}

/// Places the files of a job that are in; jobs for a world since left are dropped.
fn load_step(w: &World<'_>) {
    use std::sync::mpsc::TryRecvError;
    let jobs = with(|s| std::mem::take(&mut s.loads));
    let mut left = Vec::new();
    for job in jobs {
        let lines = match job.done.try_recv() {
            Err(TryRecvError::Empty) => {
                left.push(job);
                continue;
            }
            Ok(_) if job.world != store::world_key(w) => continue,
            Ok(read) => finish_load(w, job, read),
            Err(TryRecvError::Disconnected) => Err(format!("reading {} failed", job.label())),
        };
        let lines = lines.unwrap_or_else(|e| vec![format!("Error: {e}")]);
        if let Some(first) = lines.first() {
            with(|s| s.message = first.clone());
        }
        for l in &lines {
            say(l);
        }
    }
    with(|s| {
        left.append(&mut s.loads);
        s.loads = left;
    });
}

/// The HUD's line for each job still reading.
fn load_lines(s: &State) -> Vec<(String, u32)> {
    s.loads
        .iter()
        .map(|j| {
            let secs = j.started.elapsed().as_secs();
            (format!("Loading {} ({}) · {secs} s", j.label(), human_size(j.bytes)), 0xFFFF55)
        })
        .collect()
}

fn finish_load(w: &World<'_>, job: LoadJob, read: Vec<Result<ReadFile, String>>) -> Result<Vec<String>, String> {
    match job.then {
        Then::Load => {
            let (read, at) = first_read(&job.files, read)?;
            let (slot, mut msg) = place(w, read, at)?;
            let count = with(|s| {
                s.slots.push(slot);
                s.selected = s.slots.len() - 1;
                s.layer = LayerMode::All;
                s.moving = false;
                s.slots.len()
            });
            save_placements(w);
            if count > 1 {
                msg += &format!(" · placement {count} of {count}");
            }
            if !with(|s| std::mem::replace(&mut s.credited, true)) {
                say("Horizonmatica is based on Litematica by masa (maruohon) & Sakura-Ryoko and the Litematica Printer by aleksilassila. /hm credits");
            }
            Ok(vec![msg])
        }
        Then::Reload => {
            let (read, at) = first_read(&job.files, read)?;
            let (slot, msg) = place(w, read, at)?;
            let replaced = with(|s| {
                let same = s.slots.iter().position(|x| x.placement.file_name == slot.placement.file_name && x.placement.origin == at.0);
                match same {
                    Some(i) => {
                        let (shown, locked) = (s.slots[i].shown, s.slots[i].locked);
                        s.slots[i] = Slot { shown, locked, ..slot };
                        s.selected = i;
                    }
                    None => {
                        s.slots.push(slot);
                        s.selected = s.slots.len() - 1;
                    }
                }
                same.is_some()
            });
            save_placements(w);
            Ok(vec![if replaced { msg.replacen("Loaded", "Reloaded", 1) } else { msg }])
        }
        Then::Restore { flags, selected } => {
            let mut lines = Vec::new();
            let mut restored = Vec::new();
            for (((file, at), read), (shown, locked)) in job.files.iter().zip(read).zip(flags) {
                match read.and_then(|read| place(w, read, *at)) {
                    Ok((mut slot, _)) => {
                        slot.shown = shown;
                        slot.locked = locked;
                        restored.push(format!("{file} at {} {} {}", at.0[0], at.0[1], at.0[2]));
                        with(|s| s.slots.push(slot));
                    }
                    Err(e) => lines.push(format!("Couldn't restore {file}: {e}")),
                }
            }
            with(|s| s.selected = selected.min(s.slots.len().saturating_sub(1)));
            match restored.as_slice() {
                [] => {}
                [one] => lines.push(format!("Restored {one} (/hm unload removes it)")),
                many => lines.push(format!("Restored {} placements: {} (/hm placements)", many.len(), many.join(", "))),
            }
            Ok(lines)
        }
    }
}

fn first_read(files: &[(String, At)], read: Vec<Result<ReadFile, String>>) -> Result<(ReadFile, At), String> {
    let at = files.first().ok_or("no file")?.1;
    Ok((read.into_iter().next().ok_or("no file")??, at))
}

/// Places a file that was read: a new slot, not yet added, and the line telling what was loaded.
fn place(w: &World<'_>, read: ReadFile, at: At) -> Result<(Slot, String), String> {
    let ReadFile { file, schematic, format, from_version, upgraded, histograms } = read;
    let (origin, rotation, mirror) = at;
    let placement = Placement::placed(file, schematic, origin, rotation, mirror)?;
    let (lo, hi) = placement.bounds();
    let size = [0, 1, 2].map(|i| hi[i] - lo[i] + 1);
    let (slot, unknown) = with(|s| {
        let totals = count_totals(w, s, &placement, &histograms);
        let mut unknown: Vec<String> = Vec::new();
        for st in placement.palettes().iter().flatten() {
            if !st.is_air() && !st.is_void() && wanted_id(w, s, st).is_none() && !unknown.contains(&st.name) {
                unknown.push(st.name.clone());
            }
        }
        s.next_id += 1;
        (Slot { id: s.next_id, placement, shown: true, locked: false, totals }, unknown)
    });
    let mut msg = format!(
        "Loaded {} ({}x{}x{}, {} regions) at {} {} {}",
        display_name(&slot.placement),
        size[0],
        size[1],
        size[2],
        slot.placement.schematic.regions.len(),
        origin[0],
        origin[1],
        origin[2]
    );
    if format != Format::Litematic {
        msg.push_str(&format!(" · {}", format.name()));
    }
    if upgraded > 0 {
        msg.push_str(&format!(" · {upgraded} block states updated from data version {from_version}"));
    }
    if !unknown.is_empty() {
        msg += &format!(". Unknown blocks (skipped): {}", unknown.join(", "));
    }
    Ok((slot, msg))
}

fn load(w: &World<'_>, query: &str, at: Option<At>) -> Result<Vec<String>, String> {
    let at = match at {
        Some(a) => a,
        None => (w.feet().ok_or("not in a world")?, Rotation::None, Mirror::None),
    };
    start_load(w, vec![(query.to_string(), at)], Then::Load)
}

// ---------------------------------------------------------------- commands

/// Every subcommand with the words it takes, and the name shown for what it takes besides.
pub const WORDS: &[(&str, &[&str], Option<&str>)] = &[
    ("help", &[], None),
    ("list", &[], None),
    ("load", &[], Some("file")),
    ("reload", &[], None),
    ("unload", &["all"], Some("placement")),
    ("placements", &[], None),
    ("select", &["next", "prev"], Some("placement")),
    ("show", &["on", "off"], None),
    ("hide", &[], None),
    ("lock", &["on", "off"], None),
    ("unlock", &[], None),
    ("origin", &["here"], Some("x y z")),
    ("move", &[], Some("dx dy dz")),
    ("rotate", &["cw", "ccw", "180", "none"], None),
    ("mirror", &["none", "x", "z"], None),
    ("pos1", &["here"], Some("x y z")),
    ("pos2", &["here"], Some("x y z")),
    ("area", &["clear"], None),
    ("save", &["status", "cancel"], Some("name")),
    ("tool", &["on", "off", "mode"], Some("item")),
    ("layer", &["all", "up", "down", "here", "above", "below", "range"], Some("y")),
    ("render", &["on", "off"], None),
    ("through", &["on", "off"], None),
    ("highlight", &["missing", "wrong", "item", "off"], None),
    ("mode", &["ghost", "boxes"], None),
    ("radius", &["view"], Some("blocks")),
    ("faces", &["300k", "1m", "2m", "5m", "10m", "none"], Some("number")),
    ("fluids", &["on", "off"], None),
    ("info", &[], None),
    ("materials", &["on", "off", "copy"], None),
    ("printer", &["on", "off", "speed", "reach", "air"], None),
    ("easyplace", &["on", "off", "every", "air"], None),
    ("folder", &[], None),
    ("credits", &[], None),
];

const HELP: &[&str] = &[
    "/horizonmatica list | load <name> | unload [all] | reload | folder | info",
    "/horizonmatica placements | select <n>|next|prev | show [on|off] | hide | lock [on|off] | unlock",
    "/horizonmatica origin here | origin <x> <y> <z> | move <dx> <dy> <dz> | rotate cw|ccw|180|none | mirror none|x|z",
    "/horizonmatica pos1|pos2 [here | <x> <y> <z>] (default: the block you look at) | area [clear] | save <name> [overwrite] | save cancel|status",
    "/horizonmatica tool on|off|<item> | tool mode area|placement: the selection tool (a stick)",
    "/horizonmatica layer all | <y> | above <y> | below <y> | range <a> <b> | up | down | here",
    "/horizonmatica render on|off | through on|off | mode ghost|boxes | verify | materials [on|off] | materials copy",
    "/horizonmatica radius <blocks>|view | faces <n>|none | fluids on|off",
    "/horizonmatica highlight missing|wrong [on|off] | highlight item <item>|all | highlight off",
    "/horizonmatica printer on|off | air on|off | speed <ticks> | reach <blocks>",
    "/horizonmatica easyplace on|off | easyplace every <ticks> | easyplace air on|off: hold right click to place the block the schematic wants",
    "/horizonmatica credits: who made the original Litematica and its printer",
    "(/hm works as a short form)",
];

/// Runs `/horizonmatica …`; true when the command was ours.
pub fn command(w: &World<'_>, line: &str) -> bool {
    let line = line.trim_start_matches('/');
    let mut words = line.split_whitespace();
    match words.next() {
        Some("horizonmatica" | "hm") => {}
        _ => return false,
    }
    let args: Vec<&str> = words.collect();
    report(run(w, &args));
    true
}

fn int(s: Option<&&str>, what: &str) -> Result<i32, String> {
    s.ok_or_else(|| format!("missing {what}"))?.parse::<i32>().map_err(|_| format!("{what} must be a number"))
}

fn on_off(s: Option<&&str>, current: bool) -> Result<bool, String> {
    match s.copied() {
        None | Some("toggle") => Ok(!current),
        Some("on" | "true" | "1") => Ok(true),
        Some("off" | "false" | "0") => Ok(false),
        Some(v) => Err(format!("expected on or off, not {v}")),
    }
}

fn need_placement(s: &State) -> Result<(), String> {
    if s.slots.is_empty() { Err("no schematic loaded (/horizonmatica load <name>)".into()) } else { Ok(()) }
}

/// The selected placement, to change it.
fn selected_mut(s: &mut State) -> Result<&mut Placement, String> {
    need_placement(s)?;
    if let Some(slot) = s.sel()
        && slot.locked
    {
        return Err(format!("{} is locked (/hm unlock)", display_name(&slot.placement)));
    }
    s.placement_mut().ok_or_else(|| "no placement selected".into())
}

/// A placement by its number in `/hm placements` (from 1), else by (the start of) its name.
fn find_slot(s: &State, query: &str) -> Result<usize, String> {
    if let Ok(n) = query.parse::<usize>() {
        return (1..=s.slots.len()).contains(&n).then(|| n - 1).ok_or_else(|| format!("no placement {n} (there are {})", s.slots.len()));
    }
    let q = query.to_lowercase();
    let names: Vec<(usize, String, String)> =
        s.slots.iter().enumerate().map(|(i, x)| (i, display_name(&x.placement).to_lowercase(), x.placement.file_name.to_lowercase())).collect();
    if let Some((i, ..)) = names.iter().find(|(_, n, f)| *n == q || *f == q) {
        return Ok(*i);
    }
    let hits: Vec<usize> = names.iter().filter(|(_, n, f)| n.starts_with(&q) || f.starts_with(&q)).map(|(i, ..)| *i).collect();
    match hits.as_slice() {
        [one] => Ok(*one),
        [] => Err(format!("no placement \"{query}\" (/hm placements)")),
        _ => Err(format!("\"{query}\" matches {} placements: use its number (/hm placements)", hits.len())),
    }
}

/// One line per placement for `/hm placements`.
fn placement_lines(s: &State) -> Vec<String> {
    s.slots
        .iter()
        .enumerate()
        .map(|(i, slot)| {
            let p = &slot.placement;
            format!(
                "{}{} {} at {} {} {}{}",
                if i == s.selected { "> " } else { "  " },
                i + 1,
                display_name(p),
                p.origin[0],
                p.origin[1],
                p.origin[2],
                match (slot.shown, slot.locked) {
                    (true, false) => "",
                    (false, false) => " (hidden)",
                    (true, true) => " (locked)",
                    (false, true) => " (hidden, locked)",
                }
            )
        })
        .collect()
}

/// The block the player looks at (up to 64 blocks away), else where they stand.
fn target_block(w: &World<'_>) -> Option<[i32; 3]> {
    w.ray(64.0).map(|(pos, _)| pos).or_else(|| w.feet())
}

pub fn short(item: &str) -> &str {
    item.strip_prefix("minecraft:").unwrap_or(item)
}

pub fn run(w: &World<'_>, args: &[&str]) -> Result<Vec<String>, String> {
    ensure_settings(w);
    let sub = args.first().copied().unwrap_or("help");
    let rest = &args[args.len().min(1)..];
    let changed = |w: &World<'_>| save_placements(w);
    match sub {
        "help" | "?" => Ok(HELP.iter().map(|s| s.to_string()).collect()),
        "credits" | "about" => Ok(CREDITS.iter().map(|s| s.to_string()).collect()),
        "folder" => {
            let dir = store::schematics_dir(w)?;
            open_folder(&dir);
            Ok(vec![format!("Schematics folder: {}", dir.display())])
        }
        "list" => {
            let dir = store::schematics_dir(w)?;
            let names = horizonmatica_core::list_schematics(&dir);
            if names.is_empty() {
                return Ok(vec![format!("No schematic files (.litematic, .schem, .schematic, .nbt) in {}", dir.display())]);
            }
            Ok(vec![format!("{} schematics: {}", names.len(), names.join(", "))])
        }
        "load" => {
            if rest.is_empty() {
                return Err("usage: /horizonmatica load <name>".into());
            }
            load(w, &rest.join(" "), None)
        }
        "reload" => {
            let last = store::read_value(w, "last").ok_or("nothing loaded before")?;
            let (file, at) = parse_saved(&last).ok_or("the saved placement is unreadable")?;
            start_load(w, vec![(file, at)], Then::Reload)
        }
        "unload" | "remove" => {
            let msg = if rest.first() == Some(&"all") {
                let n = with(|s| {
                    let n = s.slots.len();
                    s.drop_all();
                    n
                });
                format!("Unloaded {n} placement{}.", if n == 1 { "" } else { "s" })
            } else {
                let (name, left) = with(|s| {
                    need_placement(s)?;
                    let i = if rest.is_empty() { s.selected } else { find_slot(s, &rest.join(" "))? };
                    let slot = s.slots.remove(i);
                    if s.selected > i || s.selected >= s.slots.len() {
                        s.selected = s.selected.saturating_sub(1);
                    }
                    if s.slots.is_empty() {
                        s.drop_all();
                    } else {
                        s.moving = false;
                    }
                    Ok::<_, String>((display_name(&slot.placement), s.slots.len()))
                })?;
                if left == 0 { format!("Unloaded {name}.") } else { format!("Unloaded {name}; {left} left (/hm placements).") }
            };
            if with(|s| s.slots.is_empty()) {
                crate::overlay::clear();
            }
            save_placements(w);
            Ok(vec![msg])
        }
        "placements" | "pl" => with(|s| {
            need_placement(s)?;
            let mut out = vec![format!("{} placements (> selected):", s.slots.len())];
            out.extend(placement_lines(s));
            Ok(out)
        }),
        "select" | "sel" => {
            let line = with(|s| {
                need_placement(s)?;
                let n = s.slots.len();
                s.selected = match rest.first().copied() {
                    None | Some("next") => (s.selected + 1) % n,
                    Some("prev" | "previous") => (s.selected + n - 1) % n,
                    Some(_) => find_slot(s, &rest.join(" "))?,
                };
                s.moving = false;
                let p = s.placement().ok_or("no placement")?;
                Ok::<_, String>(format!("Selected {} of {}: {}", s.selected + 1, n, display_name(p)))
            })?;
            save_placements(w);
            Ok(vec![line])
        }
        "lock" | "unlock" => {
            let line = with(|s| {
                need_placement(s)?;
                let i = s.selected;
                let slot = &mut s.slots[i];
                slot.locked = if sub == "unlock" { false } else { on_off(rest.first(), slot.locked)? };
                let locked = slot.locked;
                if locked {
                    s.moving = false;
                }
                let name = display_name(&s.slots[i].placement);
                Ok::<_, String>(if locked {
                    format!("{name} locked: it can't be moved, turned or mirrored (/hm unlock)")
                } else {
                    format!("{name} unlocked")
                })
            })?;
            save_placements(w);
            Ok(vec![line])
        }
        "show" | "hide" => {
            let line = with(|s| {
                need_placement(s)?;
                let i = s.selected;
                let slot = &mut s.slots[i];
                slot.shown = if sub == "hide" { false } else { on_off(rest.first(), slot.shown)? };
                if !slot.shown {
                    s.moving = false;
                }
                let slot = &s.slots[i];
                Ok::<_, String>(format!(
                    "{} {}",
                    display_name(&slot.placement),
                    if slot.shown { "shown" } else { "hidden (stays loaded; /hm show brings it back)" }
                ))
            })?;
            save_placements(w);
            Ok(vec![line])
        }
        "pos1" | "pos2" | "corner1" | "corner2" => {
            let at = match rest.first().copied() {
                None => target_block(w).ok_or("not in a world")?,
                Some("here") => w.feet().ok_or("not in a world")?,
                _ => [int(rest.first(), "x")?, int(rest.get(1), "y")?, int(rest.get(2), "z")?],
            };
            let at = horizonmatica_core::area::check_corner(at)?;
            let i = usize::from(sub.ends_with('2'));
            let size = with(|s| {
                s.area[i] = Some(at);
                s.area_size()
            });
            let mut line = format!("Corner {} at {} {} {}", i + 1, at[0], at[1], at[2]);
            if let Some(d) = size {
                line += &format!(" · area {} x {} x {} (/hm save <name>)", d[0], d[1], d[2]);
            }
            Ok(vec![line])
        }
        "area" | "selection" => {
            if matches!(rest.first().copied(), Some("clear" | "none" | "off")) {
                with(|s| s.area = [None; 2]);
                return Ok(vec!["Area selection cleared.".into()]);
            }
            let (area, size) = with(|s| (s.area, s.area_size()));
            let corner = |c: Option<[i32; 3]>| c.map_or("not set".to_string(), |c| format!("{} {} {}", c[0], c[1], c[2]));
            let mut line = format!("Area: corner 1 {}, corner 2 {}", corner(area[0]), corner(area[1]));
            if let Some(d) = size {
                line += &format!(" · {} x {} x {} = {} blocks", d[0], d[1], d[2], volume_text(d));
            }
            Ok(vec![line])
        }
        "tool" => {
            let line = with(|s| -> Result<String, String> {
                let st = &mut s.settings;
                match rest.first().copied() {
                    None => {}
                    Some("mode") => {
                        st.tool_placement = match rest.get(1).copied() {
                            Some("area" | "selection") => false,
                            Some("placement" | "placements" | "move") => true,
                            None | Some("toggle") => !st.tool_placement,
                            Some(v) => return Err(format!("tool mode area or placement, not {v}")),
                        };
                        st.tool = true;
                        return Ok(if st.tool_placement {
                            "Selection tool: placement mode · left click selects, right click moves it there, Alt+wheel moves, Ctrl+wheel turns"
                                .into()
                        } else {
                            "Selection tool: area mode · left click corner 1, right click corner 2".into()
                        });
                    }
                    Some("on" | "off" | "toggle") => st.tool = on_off(rest.first(), st.tool)?,
                    Some(item) => {
                        let item = item.to_ascii_lowercase();
                        st.tool_item = if item.contains(':') { item } else { format!("minecraft:{item}") };
                        st.tool = true;
                    }
                }
                Ok(if st.tool {
                    format!("Selection tool ON: {} in hand · left click corner 1, right click corner 2", short(&st.tool_item))
                } else {
                    "Selection tool OFF (/hm pos1 and /hm pos2 still set the corners)".into()
                })
            })?;
            save_settings(w);
            Ok(vec![line])
        }
        "save" => {
            match rest {
                ["cancel" | "stop"] => return Ok(vec![cancel_save()?]),
                ["status"] => {
                    let lines = with(|s| save_lines(w, s)).map(|(l, _)| l.into_iter().map(|(t, _)| t).collect());
                    return Ok(lines.unwrap_or_else(|| vec!["No save under way.".into()]));
                }
                _ => {}
            }
            let overwrite = rest.last() == Some(&"overwrite") && rest.len() > 1;
            let name = rest[..rest.len() - usize::from(overwrite)].join(" ");
            if name.is_empty() {
                return Err("usage: /horizonmatica save <name> [overwrite] | save cancel | save status".into());
            }
            Ok(vec![start_save(w, &name, overwrite)?])
        }
        "info" | "verify" => with(|s| {
            need_placement(s)?;
            let p = s.placement().ok_or("no placement")?;
            let (lo, hi) = p.bounds();
            let mut out = vec![format!(
                "{} ({} of {}) — origin {} {} {}, rotation {}, mirror {}, box {:?}..{:?}, layer {}",
                p.file_name,
                s.selected + 1,
                s.slots.len(),
                p.origin[0],
                p.origin[1],
                p.origin[2],
                p.rotation.name(),
                p.mirror.name(),
                lo,
                hi,
                s.layer.describe()
            )];
            match s.tally() {
                Some(t) => out.push(format!(
                    "{:.1}% done: {} correct, {} missing, {} wrong block, {} wrong state, {} extra{}",
                    t.percent(),
                    t.correct,
                    t.missing,
                    t.wrong_block,
                    t.wrong_state,
                    t.extra,
                    if s.unloaded > 0 { format!(" ({} cells in unloaded chunks)", s.unloaded) } else { String::new() }
                )),
                None => out.push("Verifier: still scanning…".into()),
            }
            Ok(out)
        }),
        "origin" => {
            let at = match rest.first().copied() {
                Some("here") | None => w.feet().ok_or("not in a world")?,
                _ => [int(rest.first(), "x")?, int(rest.get(1), "y")?, int(rest.get(2), "z")?],
            };
            with(|s| selected_mut(s)?.set_origin(at))?;
            changed(w);
            Ok(vec![format!("Origin set to {} {} {}", at[0], at[1], at[2])])
        }
        "move" | "nudge" => {
            let d = [int(rest.first(), "dx")?, int(rest.get(1), "dy")?, int(rest.get(2), "dz")?];
            let o = with(|s| {
                let p = selected_mut(s)?;
                p.nudge(d)?;
                Ok::<_, String>(p.origin)
            })?;
            changed(w);
            Ok(vec![format!("Origin now {} {} {}", o[0], o[1], o[2])])
        }
        "rotate" => {
            let r = with(|s| {
                let p = selected_mut(s)?;
                let r = match rest.first().copied() {
                    None | Some("cw") => p.rotation.then(Rotation::Clockwise90),
                    Some("ccw") => p.rotation.then(Rotation::CounterClockwise90),
                    Some(v) => Rotation::parse(v).ok_or("rotate cw, ccw, 180 or none")?,
                };
                p.set_rotation(r)?;
                Ok::<_, String>(r)
            })?;
            changed(w);
            Ok(vec![format!("Rotation: {}", r.name())])
        }
        "mirror" => {
            let m = Mirror::parse(rest.first().copied().unwrap_or("none")).ok_or("mirror none, x or z")?;
            with(|s| selected_mut(s)?.set_mirror(m))?;
            changed(w);
            Ok(vec![format!("Mirror: {}", m.name())])
        }
        "layer" => {
            let feet_y = w.feet().map_or(64, |f| f[1]);
            let l = with(|s| {
                need_placement(s)?;
                let base = s.placement().ok_or("no placement")?.bounds().0[1];
                let current = match s.layer {
                    LayerMode::All => base,
                    LayerMode::Single(y) | LayerMode::Above(y) | LayerMode::Below(y) | LayerMode::Range(y, _) => y,
                };
                s.layer = match rest.first().copied() {
                    None | Some("all") => LayerMode::All,
                    Some("up") => match s.layer {
                        LayerMode::All => LayerMode::Single(base),
                        l => l.shifted(1),
                    },
                    Some("down") => match s.layer {
                        LayerMode::All => LayerMode::Single(base),
                        l => l.shifted(-1),
                    },
                    Some("here") => LayerMode::Single(feet_y),
                    Some("above") => LayerMode::Above(rest.get(1).map_or(Ok(current), |_| int(rest.get(1), "y"))?),
                    Some("below") => LayerMode::Below(rest.get(1).map_or(Ok(current), |_| int(rest.get(1), "y"))?),
                    Some("range") => LayerMode::Range(int(rest.get(1), "min y")?, int(rest.get(2), "max y")?),
                    Some(_) => LayerMode::Single(int(rest.first(), "y")?),
                };
                Ok::<_, String>(s.layer)
            })?;
            Ok(vec![format!("Layers: {}", l.describe())])
        }
        "render" | "mode" | "through" | "xray" | "radius" | "faces" | "fluids" => {
            let view = w.host.graphics_options().render_distance as i32 * 16;
            let msg = with(|s| {
                let st = &mut s.settings;
                let msg = match sub {
                    "radius" => {
                        match rest.first().copied() {
                            Some("view") => st.radius_view = true,
                            Some(v) => {
                                st.radius = v.parse::<i32>().map_err(|_| format!("radius in blocks or view, not {v}"))?.max(16);
                                st.radius_view = false;
                            }
                            None => {}
                        }
                        if st.radius_view {
                            format!("Render radius: the view distance ({view} blocks)")
                        } else {
                            format!("Render radius: {} blocks", st.radius)
                        }
                    }
                    "faces" => {
                        if let Some(v) = rest.first() {
                            st.max_faces = settings::parse_faces(v).ok_or_else(|| format!("a number of faces (300k, 2m...) or none, not {v}"))?;
                        }
                        format!("Face limit: {}", settings::faces_text(st.max_faces))
                    }
                    "fluids" => {
                        st.hide_fluids = !on_off(rest.first(), !st.hide_fluids)?;
                        format!("Water and lava {}", if st.hide_fluids { "hidden" } else { "shown" })
                    }
                    "render" => {
                        st.render = on_off(rest.first(), st.render)?;
                        format!("Rendering {}", if st.render { "on" } else { "off" })
                    }
                    "through" | "xray" => {
                        st.through = on_off(rest.first(), st.through)?;
                        format!("See through blocks {}", if st.through { "on" } else { "off" })
                    }
                    _ => {
                        st.mode = match rest.first().copied() {
                            Some("ghost" | "ghosts") => Mode::Ghost,
                            Some("box" | "boxes") => Mode::Boxes,
                            None => {
                                if st.mode == Mode::Ghost {
                                    Mode::Boxes
                                } else {
                                    Mode::Ghost
                                }
                            }
                            Some(v) => return Err(format!("mode ghost or boxes, not {v}")),
                        };
                        format!("Mode: {}", if st.mode == Mode::Ghost { "ghost blocks" } else { "boxes" })
                    }
                };
                Ok::<_, String>(msg)
            })?;
            save_settings(w);
            Ok(vec![msg])
        }
        "highlight" | "hl" => {
            let list = material_list(w);
            let line = with(|s| -> Result<String, String> {
                match rest.first().copied() {
                    None => {}
                    Some("off") => {
                        s.settings.highlight_missing = false;
                        s.settings.highlight_wrong = false;
                        s.highlight_item = None;
                    }
                    Some("missing") => s.settings.highlight_missing = on_off(rest.get(1), s.settings.highlight_missing)?,
                    Some("wrong") => s.settings.highlight_wrong = on_off(rest.get(1), s.settings.highlight_wrong)?,
                    Some("item") => match rest[1..].join("_").to_lowercase().as_str() {
                        "" | "all" | "none" => s.highlight_item = None,
                        name => {
                            let item = if name.contains(':') { name.to_string() } else { format!("minecraft:{name}") };
                            if !list.iter().any(|m| m.item == item) {
                                return Err(format!("{name} isn't in the selected schematic's material list"));
                            }
                            s.highlight_item = Some(item);
                            if !s.settings.highlight_wrong {
                                s.settings.highlight_missing = true;
                            }
                        }
                    },
                    Some(v) => return Err(format!("highlight missing, wrong, item or off, not {v}")),
                }
                Ok(highlight_text(s))
            })?;
            save_settings(w);
            Ok(vec![line])
        }
        "materials" | "mats" => {
            if rest.first() == Some(&"copy") {
                return copy_materials(w);
            }
            let line = with(|s| -> Result<String, String> {
                s.settings.materials_hud = on_off(rest.first(), s.settings.materials_hud)?;
                Ok(format!("Material list in the HUD: {}", if s.settings.materials_hud { "on" } else { "off" }))
            })?;
            save_settings(w);
            Ok(vec![line])
        }
        "printer" | "print" => {
            let msg = with(|s| {
                let st = &mut s.settings;
                let msg = match rest.first().copied() {
                    Some("speed") => {
                        st.printer.interval = int(rest.get(1), "ticks")?.clamp(1, 20) as u32;
                        format!("Printer: one block every {} ticks", st.printer.interval)
                    }
                    Some("reach") => {
                        let r: f64 = rest.get(1).ok_or("missing reach")?.parse().map_err(|_| "reach must be a number")?;
                        st.printer.reach = r.clamp(1.0, 6.0);
                        format!("Printer reach: {:.1}", st.printer.reach)
                    }
                    Some("air") => {
                        st.printer.air_place = on_off(rest.get(1), st.printer.air_place)?;
                        format!("Printer air place {}", if st.printer.air_place { "ON — places without a neighbouring block" } else { "OFF" })
                    }
                    v => {
                        st.printer_on = on_off(v.as_ref(), st.printer_on)?;
                        s.recent.clear();
                        format!("Printer {}", if s.settings.printer_on { "ON — places missing blocks within reach" } else { "OFF" })
                    }
                };
                Ok::<_, String>(msg)
            })?;
            save_settings(w);
            Ok(vec![msg])
        }
        "easyplace" | "easy" => {
            let msg = with(|s| {
                let st = &mut s.settings;
                let msg = match rest.first().copied() {
                    Some("every") => {
                        st.easy_interval = int(rest.get(1), "ticks")?.clamp(1, 10) as u32;
                        format!("Easy place: one block every {} ticks", st.easy_interval)
                    }
                    Some("air") => {
                        st.easy_air = on_off(rest.get(1), st.easy_air)?;
                        format!("Easy air place {}", if st.easy_air { "ON — ghost blocks need no neighbouring block" } else { "OFF" })
                    }
                    v => {
                        st.easy_place = on_off(v.as_ref(), st.easy_place)?;
                        format!(
                            "Easy place {}",
                            if st.easy_place { "ON — hold right click to place what the schematic wants" } else { "OFF" }
                        )
                    }
                };
                Ok::<_, String>(msg)
            })?;
            save_settings(w);
            Ok(vec![msg])
        }
        other => Err(format!("unknown subcommand {other}; /horizonmatica help")),
    }
}

/// Opens a folder in the system's file manager.
fn open_folder(dir: &std::path::Path) {
    #[cfg(target_os = "windows")]
    let program = "explorer";
    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let program = "xdg-open";
    let _ = std::process::Command::new(program).arg(dir).spawn();
}

// ---------------------------------------------------------------- keys

/// Everything a key can do: (id, label, default key). Registered as the game's key mappings
/// (Options > Controls > Horizonmatica); the id is the action's (`action`).
pub const BINDS: &[(&str, &str, &str)] = &[
    ("open_menu", "Open the Horizonmatica menu", "key.keyboard.m"),
    ("toggle_render", "Toggle schematic rendering", "key.keyboard.v"),
    ("layer_up", "Layer up", "key.keyboard.page.up"),
    ("layer_down", "Layer down", "key.keyboard.page.down"),
    ("layer_mode", "Cycle layer mode", "key.keyboard.home"),
    ("toggle_printer", "Printer on/off", "key.keyboard.k"),
    ("toggle_easy_place", "Easy place on/off", "key.keyboard.unknown"),
    ("toggle_materials", "Material list on/off", "key.keyboard.j"),
    ("move_mode", "Move mode", "key.keyboard.n"),
    ("toggle_tool", "Selection tool on/off", "key.keyboard.unknown"),
    ("reload", "Reload the last schematic", "key.keyboard.unknown"),
    ("rotate_cw", "Rotate clockwise", "key.keyboard.unknown"),
    ("rotate_ccw", "Rotate counter-clockwise", "key.keyboard.unknown"),
    ("mirror_next", "Mirror (next axis)", "key.keyboard.unknown"),
    ("here", "Move the placement to me", "key.keyboard.unknown"),
    ("show_toggle", "Show or hide the placement", "key.keyboard.unknown"),
    ("select_next", "Select next placement", "key.keyboard.unknown"),
    ("select_prev", "Select previous placement", "key.keyboard.unknown"),
    ("unload", "Unload this placement", "key.keyboard.unknown"),
    ("unload_all", "Unload all placements", "key.keyboard.unknown"),
    ("area_pos1", "Area corner 1 at my position", "key.keyboard.unknown"),
    ("area_pos2", "Area corner 2 at my position", "key.keyboard.unknown"),
    ("area_clear", "Clear the area", "key.keyboard.unknown"),
    ("materials_copy", "Copy the material list", "key.keyboard.unknown"),
    ("folder", "Open the schematics folder", "key.keyboard.unknown"),
];

/// The next layer mode (all, this layer, above, below), as the key cycles them.
pub fn next_layer_mode() -> &'static str {
    with(|s| match s.layer {
        LayerMode::All => "here",
        LayerMode::Single(_) => "above",
        LayerMode::Above(_) => "below",
        _ => "all",
    })
}

/// A key's or a menu row's action.
pub fn action(w: &World<'_>, action: &str) {
    let r = match action {
        "move_mode" => {
            toggle_move_mode(w);
            return;
        }
        "credits_chat" => {
            for line in CREDITS {
                with(|s| s.out.push((*line).to_string()));
            }
            return;
        }
        "toggle_materials" => {
            // The HUD list only: don't fill the chat each press.
            with(|s| s.settings.materials_hud = !s.settings.materials_hud);
            save_settings(w);
            return;
        }
        "mirror_next" => {
            let next = with(|s| match s.placement().map(|p| p.mirror) {
                // x flips x (FRONT_BACK), z flips z (LEFT_RIGHT).
                Some(Mirror::None) => "x",
                Some(Mirror::FrontBack) => "z",
                _ => "none",
            });
            run(w, &["mirror", next])
        }
        "layer_next" | "layer_mode" => {
            let next = next_layer_mode();
            run(w, &["layer", next])
        }
        "folder" => run(w, &["folder"]),
        "reload" => run(w, &["reload"]),
        "here" => run(w, &["origin", "here"]),
        "rotate_cw" => run(w, &["rotate", "cw"]),
        "rotate_ccw" => run(w, &["rotate", "ccw"]),
        "unload" => run(w, &["unload"]),
        "unload_all" => run(w, &["unload", "all"]),
        "select_next" => run(w, &["select", "next"]),
        "show_toggle" => run(w, &["show", "toggle"]),
        "printer_toggle" | "toggle_printer" => run(w, &["printer"]),
        "toggle_render" => run(w, &["render"]),
        "layer_up" => run(w, &["layer", "up"]),
        "layer_down" => run(w, &["layer", "down"]),
        "toggle_easy_place" => run(w, &["easyplace"]),
        "toggle_tool" => {
            let on = with(|s| s.settings.tool);
            run(w, &["tool", if on { "off" } else { "on" }])
        }
        "select_prev" => run(w, &["select", "prev"]),
        "area_pos1" => run(w, &["pos1", "here"]),
        "area_pos2" => run(w, &["pos2", "here"]),
        "area_clear" => run(w, &["area", "clear"]),
        "materials" | "materials_copy" => run(w, &["materials", "copy"]),
        other => match other.strip_prefix("load:") {
            Some(file) => run(w, &["load", file]),
            None => return,
        },
    };
    report(r);
}

/// The registered keys pressed since the last tick, in order (move mode takes Page up/down and
/// Home for itself).
pub fn key_clicked(w: &World<'_>, id: &str) {
    if with(|s| s.moving) && matches!(id, "layer_up" | "layer_down" | "layer_mode") {
        return;
    }
    if id == "open_menu" {
        crate::menu::open(w);
        return;
    }
    action(w, id);
}

// ---------------------------------------------------------------- move mode

pub fn toggle_move_mode(w: &World<'_>) {
    if with(|s| !s.moving && s.sel().is_some_and(|x| x.locked)) {
        say("Move mode: the placement is locked (/hm unlock)");
        return;
    }
    let on = with(|s| {
        s.placement()?;
        s.moving = !s.moving;
        s.wheel = 0.0;
        Some(s.moving)
    });
    match on {
        None => say("Move mode: no schematic loaded"),
        Some(true) => say(
            "Move mode ON: wheel = along your view, arrows = horizontal, Page up/down = up/down, R = rotate, Home = to you, Shift = 5 blocks. N or Enter: done",
        ),
        Some(false) => {
            save_placements(w);
            say("Move mode OFF")
        }
    }
}

/// GLFW's modifier bits, as the input hooks give them.
const SHIFT: u32 = 1;
const CONTROL: u32 = 2;
const ALT: u32 = 4;

/// Blocks per step: 5 with Shift held.
fn move_step(modifiers: u32) -> i32 {
    if modifiers & SHIFT != 0 { 5 } else { 1 }
}

/// Minecraft's view vector for a yaw and pitch in degrees.
pub fn view_vector(yaw: f32, pitch: f32) -> [f64; 3] {
    let (yaw, pitch) = (f64::from(yaw).to_radians(), f64::from(pitch).to_radians());
    [-yaw.sin() * pitch.cos(), -pitch.sin(), yaw.cos() * pitch.cos()]
}

/// The unit axis closest to `dir`.
fn dominant(dir: [f64; 3]) -> [i32; 3] {
    let i = (0..3).max_by(|&a, &b| dir[a].abs().total_cmp(&dir[b].abs())).unwrap_or(0);
    let mut out = [0; 3];
    out[i] = if dir[i] < 0.0 { -1 } else { 1 };
    out
}

/// Moves the placement by `d` without a chat line (the HUD shows the origin). False when it is
/// locked.
fn shift(d: [i32; 3]) -> bool {
    with(|s| {
        if s.sel().is_some_and(|x| x.locked) {
            return false;
        }
        // At the edge of the world it stays put.
        if s.placement_mut().is_some_and(|p| p.nudge(d).is_ok()) {
            s.scan = Scan::default();
        }
        true
    })
}

/// A key pressed while playing (`input_key`): move mode's keys.
pub fn raw_key(w: &World<'_>, key: &str, modifiers: u32) {
    if !with(|s| s.moving) {
        return;
    }
    let k = move_step(modifiers);
    let Some((yaw, _)) = w.look() else { return };
    let view = view_vector(yaw, 0.0);
    let fwd = dominant([view[0], 0.0, view[2]]);
    let right = [-fwd[2], 0, fwd[0]];
    let d = match key {
        "key.keyboard.up" => fwd,
        "key.keyboard.down" => fwd.map(|v| -v),
        "key.keyboard.right" => right,
        "key.keyboard.left" => right.map(|v| -v),
        "key.keyboard.page.up" => [0, 1, 0],
        "key.keyboard.page.down" => [0, -1, 0],
        "key.keyboard.r" => {
            report(run(w, &["rotate", "cw"]).map(|_| Vec::new()));
            return;
        }
        "key.keyboard.home" => {
            report(run(w, &["origin", "here"]).map(|_| Vec::new()));
            return;
        }
        "key.keyboard.enter" | "key.keyboard.keypad.enter" => {
            toggle_move_mode(w);
            return;
        }
        _ => return,
    };
    if !shift(d.map(|v| v * k)) {
        say("The placement is locked (/hm unlock)");
    }
}

/// The mouse wheel while playing: in move mode it moves the placement along the axis you look
/// along; with the selection tool and Alt or Ctrl, it moves the area or the placement. Whether
/// the wheel was taken (the hotbar stays put).
pub fn wheel(w: &World<'_>, steps: f32, modifiers: u32) -> bool {
    if !with(|s| s.moving && s.placement().is_some()) {
        return tool_wheel(w, steps, modifiers);
    }
    let n = wheel_steps(steps);
    if n == 0 {
        return true;
    }
    let Some((yaw, pitch)) = w.look() else { return true };
    let axis = dominant(view_vector(yaw, pitch));
    let k = n.saturating_mul(move_step(modifiers));
    if !shift(axis.map(|v| v.saturating_mul(k))) {
        say("The placement is locked (/hm unlock)");
    }
    true
}

/// Whole wheel steps (touchpads send fractions; the rest is kept).
fn wheel_steps(steps: f32) -> i32 {
    with(|s| {
        s.wheel += steps;
        let n = s.wheel.trunc();
        s.wheel -= n;
        n as i32
    })
}

// ---------------------------------------------------------------- the selection tool

/// How far middle click looks for a corner.
const PICK_RANGE: f64 = 512.0;

/// Whether the selection tool is in the main hand (and switched on).
pub fn tool_held(w: &World<'_>) -> bool {
    let item = with(|s| s.settings.tool.then(|| s.settings.tool_item.clone()));
    item.is_some_and(|item| w.held().is_some_and(|h| h == item))
}

/// The face of the block `pos` that the point `hit` lies on.
fn face_of(pos: [i32; 3], hit: [f64; 3]) -> [i32; 3] {
    let mut best = (f64::MAX, [0, 1, 0]);
    for i in 0..3 {
        let lo = (hit[i] - f64::from(pos[i])).abs();
        let hi = (hit[i] - f64::from(pos[i] + 1)).abs();
        let mut f = [0; 3];
        if lo < best.0 {
            f[i] = -1;
            best = (lo, f);
        }
        let mut f = [0; 3];
        if hi < best.0 {
            f[i] = 1;
            best = (hi, f);
        }
    }
    best.1
}

/// Where a tool click puts a corner: the first block along the view within the tool's reach
/// (water and lava count when the setting says so), or the block in front of the face hit.
fn tool_target(w: &World<'_>) -> Option<[i32; 3]> {
    let (range, front, fluids) = with(|s| (s.settings.tool_range, s.settings.tool_front, s.settings.tool_fluids));
    let (origin, dir) = w.aim()?;
    let hit = w.ray(range).map(|(pos, point)| {
        let length = (0..3).map(|i| (point[i] - origin[i]).powi(2)).sum::<f64>().sqrt();
        let face = face_of(pos, point);
        (pos, [0, 1, 2].map(|i| pos[i] + face[i]), length)
    });
    let hit = match fluids.then(|| first_fluid(w, origin, dir, hit.map_or(range, |h| h.2))).flatten() {
        Some((cell, before)) => (cell, before),
        None => hit.map(|h| (h.0, h.1))?,
    };
    Some(if front { hit.1 } else { hit.0 })
}

/// The first water or lava cell along the ray within `reach`, and the cell before it.
fn first_fluid(w: &World<'_>, origin: [f64; 3], dir: [f64; 3], reach: f64) -> Option<([i32; 3], [i32; 3])> {
    let cell_at = |t: f64| [0, 1, 2].map(|i| (origin[i] + dir[i] * t).floor() as i32);
    let mut last = cell_at(0.0);
    let mut t = 0.0;
    while t < reach {
        t += 0.05;
        let cell = cell_at(t);
        if cell == last {
            continue;
        }
        if let Some(id) = w.block(cell) {
            let fluid = with(|s| {
                let st = s.blocks.state(w, id);
                fluid_of(&st) != 0 || st.prop("waterlogged") == Some("true")
            });
            if fluid {
                return Some((cell, last));
            }
        }
        last = cell;
    }
    None
}

/// Where the ray from `o` along `d` enters the box `lo..hi`, if it does.
fn ray_box(o: [f64; 3], d: [f64; 3], lo: [f64; 3], hi: [f64; 3]) -> Option<f64> {
    let (mut near, mut far) = (0.0f64, PICK_RANGE);
    for i in 0..3 {
        if d[i].abs() < 1e-9 {
            if o[i] < lo[i] || o[i] > hi[i] {
                return None;
            }
            continue;
        }
        let (a, b) = ((lo[i] - o[i]) / d[i], (hi[i] - o[i]) / d[i]);
        near = near.max(a.min(b));
        far = far.min(a.max(b));
        if near > far {
            return None;
        }
    }
    Some(near)
}

/// A mouse button while playing (GLFW: 0 left, 1 right, 2 middle). With the selection tool in
/// hand: left click puts corner 1 on the block looked at, right click corner 2, middle click
/// picks what the wheel moves. The game doesn't see these clicks. Whether the click was taken.
pub fn mouse(w: &World<'_>, button: i32, pressed: bool, modifiers: u32) -> bool {
    let Ok(b) = usize::try_from(button) else { return false };
    if b > 2 {
        return false;
    }
    // A release goes where its press went.
    if !pressed {
        return with(|s| std::mem::take(&mut s.tool_down[b]));
    }
    if !tool_held(w) {
        return false;
    }
    with(|s| s.tool_down[b] = true);
    // Ctrl + middle click: the other mode.
    if b == 2 && modifiers & CONTROL != 0 {
        report(run(w, &["tool", "mode", "toggle"]));
        return true;
    }
    if with(|s| s.settings.tool_placement) {
        placement_click(w, b);
        return true;
    }
    if b < 2 {
        if let Some(at) = tool_target(w) {
            with(|s| {
                s.area[b] = Some(horizonmatica_core::area::clamp_corner(at));
                s.grab = Grab::Corner(b);
            });
        }
        return true;
    }
    let Some((origin, dir)) = w.aim() else { return true };
    with(|s| {
        let corner = |i: usize| {
            let c = s.area[i]?.map(f64::from);
            ray_box(origin, dir, c.map(|v| v - 0.05), c.map(|v| v + 1.05)).map(|t| (t, i))
        };
        let looked = [corner(0), corner(1)].into_iter().flatten().min_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, i)| i);
        s.grab = match looked {
            // The corner already picked, picked again: back to the box.
            Some(i) if s.grab != Grab::Corner(i) => Grab::Corner(i),
            _ => Grab::Box,
        };
    });
    true
}

/// The placement looked at: the nearest box the view enters (inside two, the smaller).
fn looked_placement(w: &World<'_>) -> Option<usize> {
    let (origin, dir) = w.aim()?;
    with(|s| {
        s.slots
            .iter()
            .enumerate()
            .filter(|(_, x)| x.shown)
            .filter_map(|(i, x)| {
                let (lo, hi) = x.placement.bounds();
                let t = ray_box(origin, dir, lo.map(f64::from), hi.map(|v| f64::from(v) + 1.0))?;
                let volume: i64 = (0..3).map(|k| i64::from(hi[k] - lo[k] + 1)).product();
                Some((t, volume, i))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
            .map(|(.., i)| i)
    })
}

/// A click with the tool in placement mode: left (or middle) selects the placement looked at,
/// right puts the selected one's origin on the block looked at.
fn placement_click(w: &World<'_>, b: usize) {
    if b == 1 {
        let Some(at) = tool_target(w) else { return };
        report(run(w, &["origin", &at[0].to_string(), &at[1].to_string(), &at[2].to_string()]).map(|_| Vec::new()));
        return;
    }
    match looked_placement(w) {
        Some(i) if with(|s| s.selected != i) => report(run(w, &["select", &(i + 1).to_string()]).map(|_| Vec::new())),
        Some(_) => {}
        None => say("No placement there (look at one's box)"),
    }
}

/// The wheel with the tool in placement mode: Alt + wheel moves the selected placement along
/// the axis you look along, Ctrl + wheel turns it (up: clockwise). Shift: 5 blocks.
fn placement_wheel(w: &World<'_>, steps: f32, modifiers: u32) -> bool {
    let (alt, ctrl) = (modifiers & ALT != 0, modifiers & CONTROL != 0);
    if !(alt || ctrl) || with(|s| s.placement().is_none()) {
        return false;
    }
    let n = wheel_steps(steps);
    if n == 0 {
        return true;
    }
    if ctrl {
        for _ in 0..n.unsigned_abs() {
            if let Err(e) = run(w, &["rotate", if n > 0 { "cw" } else { "ccw" }]) {
                say(&format!("Error: {e}"));
                break;
            }
        }
        return true;
    }
    let Some((yaw, pitch)) = w.look() else { return true };
    let axis = dominant(view_vector(yaw, pitch));
    let k = n.saturating_mul(move_step(modifiers));
    if shift(axis.map(|v| v.saturating_mul(k))) {
        save_placements(w);
    } else {
        say("The placement is locked (/hm unlock)");
    }
    true
}

/// The wheel with the selection tool in hand: Alt + wheel moves the picked corner (or the whole
/// box) along the axis you look along, Ctrl + wheel pushes the box's face in that direction out
/// (wheel up) or in (wheel down). Shift: 5 blocks. Without Alt or Ctrl the wheel is the hotbar's.
fn tool_wheel(w: &World<'_>, steps: f32, modifiers: u32) -> bool {
    let (alt, ctrl) = (modifiers & ALT != 0, modifiers & CONTROL != 0);
    if !(alt || ctrl) || !tool_held(w) {
        return false;
    }
    if with(|s| s.settings.tool_placement) {
        return placement_wheel(w, steps, modifiers);
    }
    if with(|s| s.area.iter().all(Option::is_none)) {
        return false;
    }
    let n = wheel_steps(steps);
    if n == 0 {
        return true;
    }
    let Some((yaw, pitch)) = w.look() else { return true };
    let axis = dominant(view_vector(yaw, pitch));
    let k = n.saturating_mul(move_step(modifiers));
    with(|s| {
        if ctrl {
            grow(s, axis, k);
        } else {
            let d = axis.map(|a| a.saturating_mul(k));
            let moved = |c: Option<[i32; 3]>| c.map(|c| horizonmatica_core::area::offset_corner(c, d));
            match s.grab {
                Grab::Corner(i) => s.area[i] = moved(s.area[i]),
                Grab::Box => s.area = s.area.map(moved),
            }
        }
    });
    true
}

/// Pushes the face of the box that `axis` points at by `k` blocks: out when `k` is positive, in
/// when negative (never past the opposite face).
fn grow(s: &mut State, axis: [i32; 3], k: i32) {
    let [Some(a), Some(b)] = s.area else { return };
    let i = (0..3).find(|&i| axis[i] != 0).unwrap_or(0);
    let sign = axis[i];
    let f = if (a[i] - b[i]) * sign > 0 || (a[i] == b[i] && sign > 0) { 0 } else { 1 };
    let (c, other) = if f == 0 { (a, b[i]) } else { (b, a[i]) };
    let mut d = [0; 3];
    d[i] = sign.saturating_mul(k);
    let mut c = horizonmatica_core::area::offset_corner(c, d);
    if (i64::from(c[i]) - i64::from(other)) * i64::from(sign) < 0 {
        c[i] = other;
    }
    s.area[f] = Some(c);
}

/// The HUD's lines while the selection tool is in hand.
fn tool_lines(s: &State) -> Vec<(String, u32)> {
    if s.settings.tool_placement {
        let selected = match s.sel() {
            Some(slot) => {
                let p = &slot.placement;
                format!(
                    "Selected: {} at {} {} {} · {}{}",
                    display_name(p),
                    p.origin[0],
                    p.origin[1],
                    p.origin[2],
                    p.rotation.name(),
                    if slot.locked { " · LOCKED" } else { "" }
                )
            }
            None => "No placement loaded".into(),
        };
        return vec![
            ("Placement tool: left click selects the placement you look at · right click moves it there".into(), 0x55FFFF),
            (selected, 0xE0E0E0),
            ("Alt+wheel moves it · Ctrl+wheel turns it · Shift: 5 · Ctrl+middle click: area mode".into(), 0xAAAAAA),
        ];
    }
    let c = |i: usize| s.area[i].map_or("not set".to_string(), |c| format!("{} {} {}", c[0], c[1], c[2]));
    let mut corners = format!("Corner 1: {} · corner 2: {}", c(0), c(1));
    if let Some(d) = s.area_size() {
        corners += &format!(" · {} x {} x {}", d[0], d[1], d[2]);
    }
    let what = match s.grab {
        Grab::Box => "the box".to_string(),
        Grab::Corner(i) => format!("corner {}", i + 1),
    };
    vec![
        ("Selection tool: left click corner 1 · right click corner 2 · middle click picks a corner".into(), 0xFFFF55),
        (corners, 0xE0E0E0),
        (format!("Alt+wheel moves {what} · Ctrl+wheel grows/shrinks where you look · Shift: 5 · Ctrl+middle click: placement mode"), 0xAAAAAA),
    ]
}

// ---------------------------------------------------------------- saving an area

/// Most positions one save takes: as many as a file may hold to be loaded again (2^31).
const MAX_SAVE: u64 = horizonmatica_core::schematic::MAX_VOLUME;
/// Time a save may take per tick.
const SAVE_BUDGET: std::time::Duration = std::time::Duration::from_millis(4);

/// A save under way: the area is read chunk by chunk as the chunks are loaded (as Litematica's
/// task does), a few per tick; each chunk as it was when it was read.
pub struct SaveJob {
    name: String,
    file: String,
    path: std::path::PathBuf,
    pub lo: [i32; 3],
    pub hi: [i32; 3],
    builder: horizonmatica_core::schematic::RegionBuilder,
    /// Game state → palette index.
    ids: HashMap<StateId, u32>,
    /// Chunk columns not read yet.
    pending: std::collections::BTreeSet<(i32, i32)>,
    total: usize,
    /// Of `pending`, those not loaded, nearest first (updated every tick).
    pub unloaded: Vec<(i32, i32)>,
    started: std::time::Instant,
    /// The world (`world_key`) the area is in.
    world: Option<String>,
}

/// A file being written by its own thread: its name and the line it reports when done.
struct SaveWrite {
    file: String,
    done: std::sync::mpsc::Receiver<String>,
}

/// The file name (without `.litematic`) for a schematic called `name`: characters no file system
/// takes become `_`.
pub fn file_stem(name: &str) -> Result<String, String> {
    let name = name.trim();
    let name = if name.to_ascii_lowercase().ends_with(".litematic") { &name[..name.len() - 10] } else { name };
    let stem: String = name
        .chars()
        .map(|c| if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') { '_' } else { c })
        .collect();
    let stem = stem.trim_end_matches(['.', ' ']).trim().to_string();
    if stem.is_empty() {
        return Err("give it a name".into());
    }
    if stem.chars().count() > 100 {
        return Err("that name is too long".into());
    }
    Ok(stem)
}

/// Starts saving the area selection as `<name>.litematic` in the schematics folder: one region
/// with every block and its state, as Litematica writes it (version 7). Block entity data and
/// entities aren't known to the client this way and aren't saved.
pub fn start_save(w: &World<'_>, name: &str, overwrite: bool) -> Result<String, String> {
    if let Some(file) = with(|s| s.save.as_ref().map(|j| j.file.clone()).or_else(|| s.save_write.as_ref().map(|w| w.file.clone()))) {
        return Err(format!("{file} is still being saved (/hm save cancel stops it)"));
    }
    let (lo, hi) = with(|s| s.area_box()).ok_or("set both corners first (the selection tool, or /hm pos1 and /hm pos2)")?;
    let size = horizonmatica_core::area::size(lo, hi).ok_or("the area is too large to save")?;
    match horizonmatica_core::area::volume(size) {
        Some(volume) if volume <= MAX_SAVE => {}
        _ => return Err(format!("the area has {} blocks; at most {MAX_SAVE} can be saved at once", volume_text(size))),
    }
    let stem = file_stem(name)?;
    let file = format!("{stem}.litematic");
    let path = store::schematics_dir(w)?.join(&file);
    if path.exists() && !overwrite {
        return Err(format!("{file} exists: /hm save {name} overwrite replaces it"));
    }
    let pending: std::collections::BTreeSet<(i32, i32)> =
        (lo[0] >> 4..=hi[0] >> 4).flat_map(|cx| (lo[2] >> 4..=hi[2] >> 4).map(move |cz| (cx, cz))).collect();
    let total = pending.len();
    let job = SaveJob {
        name: name.trim().to_string(),
        file: file.clone(),
        path,
        lo,
        hi,
        builder: horizonmatica_core::schematic::RegionBuilder::new(size),
        ids: HashMap::new(),
        pending,
        total,
        unloaded: Vec::new(),
        started: std::time::Instant::now(),
        world: store::world_key(w),
    };
    with(|s| s.save = Some(job));
    Ok(format!(
        "Saving {file}: {} x {} x {} in {total} chunk{} (the HUD shows the progress)",
        size[0],
        size[1],
        size[2],
        if total == 1 { "" } else { "s" }
    ))
}

/// The save's work for this tick: a finished file reported; chunks read while the time lasts;
/// the last one in, the file handed to a thread of its own to write.
fn save_step(w: &World<'_>) {
    if let Some(line) = with(|s| s.save_write.as_ref().and_then(|sw| sw.done.try_recv().ok())) {
        with(|s| {
            s.save_write = None;
            s.message = line.clone();
        });
        say(&line);
    }
    let Some(mut job) = with(|s| s.save.take()) else { return };
    if job.world != store::world_key(w) {
        let line = format!("Saving {} stopped: you left the world it is in", job.file);
        with(|s| s.message = line.clone());
        say(&line);
        return;
    }
    let start = std::time::Instant::now();
    let loaded: Vec<(i32, i32)> = job.pending.iter().copied().filter(|&(cx, cz)| chunk_ready(w, &job, cx, cz)).collect();
    for c in loaded {
        if start.elapsed() > SAVE_BUDGET {
            break;
        }
        save_chunk(w, &mut job, c);
        job.pending.remove(&c);
    }
    if job.pending.is_empty() {
        finish_save(w, job);
        return;
    }
    let me = w.feet().unwrap_or(job.lo);
    let mut unloaded: Vec<(i32, i32)> = job.pending.iter().copied().filter(|&(cx, cz)| !chunk_ready(w, &job, cx, cz)).collect();
    unloaded.sort_by_key(|&(cx, cz)| {
        let (dx, dz) = (i64::from(cx * 16 + 8 - me[0]), i64::from(cz * 16 + 8 - me[2]));
        dx * dx + dz * dz
    });
    job.unloaded = unloaded;
    with(|s| s.save = Some(job));
}

/// Whether chunk column `cx`, `cz` is loaded where the area is (its corner cell at the area's
/// bottom is read).
fn chunk_ready(w: &World<'_>, job: &SaveJob, cx: i32, cz: i32) -> bool {
    let x = (cx * 16).max(job.lo[0]);
    let z = (cz * 16).max(job.lo[2]);
    w.loaded([x, job.lo[1], z])
}

/// Reads one chunk column's part of the area.
fn save_chunk(w: &World<'_>, job: &mut SaveJob, (cx, cz): (i32, i32)) {
    let (lo, hi) = (job.lo, job.hi);
    let (x0, x1) = ((cx * 16).max(lo[0]), (cx * 16 + 15).min(hi[0]));
    let (z0, z1) = ((cz * 16).max(lo[2]), (cz * 16 + 15).min(hi[2]));
    for y in lo[1]..=hi[1] {
        for z in z0..=z1 {
            for x in x0..=x1 {
                let Some(id) = w.block([x, y, z]) else { continue };
                let i = match job.ids.get(&id) {
                    Some(&i) => i,
                    None => {
                        let state = with(|s| s.blocks.state(w, id));
                        let i = job.builder.intern(state);
                        job.ids.insert(id, i);
                        i
                    }
                };
                if i != 0 {
                    job.builder.set([x - lo[0], y - lo[1], z - lo[2]], i);
                }
            }
        }
    }
}

/// Every chunk is in: the schematic is built and written by a thread of its own.
fn finish_save(w: &World<'_>, job: SaveJob) {
    let SaveJob { name, file, path, lo, hi, builder, started, .. } = job;
    let size = horizonmatica_core::area::size(lo, hi).unwrap_or([1; 3]);
    let volume = horizonmatica_core::area::volume(size).and_then(|v| i64::try_from(v).ok()).unwrap_or(0);
    let (blocks, states) = (builder.non_air(), builder.states());
    let stem = file.trim_end_matches(".litematic").to_string();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64);
    let author = w.player.and_then(|p| p.name(w.host)).unwrap_or_default();
    let schematic = Schematic {
        version: 7,
        data_version: i64::from(crate::DATA_VERSION),
        metadata: horizonmatica_core::schematic::Metadata {
            name,
            author,
            description: String::new(),
            region_count: 1,
            total_volume: volume,
            total_blocks: blocks as i64,
            enclosing_size: size,
            time_created: now,
            time_modified: now,
        },
        regions: vec![builder.finish(&stem, Vec::new())],
    };
    let (tx, rx) = std::sync::mpsc::channel();
    let name = file.clone();
    std::thread::spawn(move || {
        let tmp = path.with_file_name(format!("{name}.tmp"));
        let written = std::fs::write(&tmp, schematic.to_bytes()).map_err(|e| format!("{}: {e}", tmp.display())).and_then(|()| {
            std::fs::rename(&tmp, &path).map_err(|e| {
                let _ = std::fs::remove_file(&tmp);
                format!("{}: {e}", path.display())
            })
        });
        let line = match written {
            Ok(()) => format!(
                "Saved {name}: {} x {} x {}, {blocks} blocks, {states} block states ({:.1} s)",
                size[0],
                size[1],
                size[2],
                started.elapsed().as_secs_f32()
            ),
            Err(e) => format!("Error: couldn't write {name}: {e}"),
        };
        let _ = tx.send(line);
    });
    with(|s| s.save_write = Some(SaveWrite { file, done: rx }));
}

fn cancel_save() -> Result<String, String> {
    with(|s| match s.save.take() {
        Some(job) => Ok(format!("Saving {} stopped; nothing was written", job.file)),
        None if s.save_write.is_some() => Err("the file is being written; that can't be stopped".into()),
        None => Err("no save under way".into()),
    })
}

/// "north-east" and the like, for a step of `dx`, `dz` (north is -z).
fn compass(dx: i32, dz: i32) -> &'static str {
    const NAMES: [&str; 8] = ["east", "south-east", "south", "south-west", "west", "north-west", "north", "north-east"];
    let angle = f64::from(dz).atan2(f64::from(dx)).to_degrees().rem_euclid(360.0);
    NAMES[((angle + 22.5) / 45.0) as usize % 8]
}

/// The save's progress: lines for the HUD and `/hm save status`, and how far it is (0 to 1).
pub fn save_lines(w: &World<'_>, s: &State) -> Option<(Vec<(String, u32)>, f32)> {
    if let Some(sw) = &s.save_write {
        return Some((vec![(format!("Writing {}…", sw.file), 0xFFAA00)], 1.0));
    }
    let job = s.save.as_ref()?;
    let done = job.total - job.pending.len();
    let mut lines = vec![(format!("Saving {}: {done} of {} chunks", job.file, job.total), 0xFFAA00)];
    if let Some(&(cx, cz)) = job.unloaded.first() {
        let me = w.feet().unwrap_or(job.lo);
        let (dx, dz) = (cx * 16 + 8 - me[0], cz * 16 + 8 - me[2]);
        let far = f64::from(dx).hypot(f64::from(dz)).round();
        lines.push((
            format!("{} not loaded · nearest: {far} blocks {} ({} {})", job.unloaded.len(), compass(dx, dz), cx * 16 + 8, cz * 16 + 8),
            0xFFAA00,
        ));
    }
    Some((lines, done as f32 / job.total.max(1) as f32))
}

// ---------------------------------------------------------------- per tick

/// Each client tick in a world.
pub fn tick(w: &World<'_>) {
    if !w.in_world() {
        return;
    }
    ensure_settings(w);
    with(|s| s.ticks += 1);
    restore_step(w);
    load_step(w);
    with(State::sync_rev);
    scan_step(w);
    save_step(w);
}

/// The verifier, over and over, for the selected placement alone (shown or not): its progress
/// in the HUD and its material list. Another placement selected starts it over.
fn scan_step(w: &World<'_>) {
    with(|s| {
        let Some(slot) = s.sel() else { return };
        let (sel, key) = (s.selected, (slot.id, slot.placement.revision));
        let (lo, hi) = slot.placement.bounds();
        let size = [0, 1, 2].map(|i| (hi[i] - lo[i] + 1) as u64);
        let volume = size[0] * size[1] * size[2];
        if s.scan.key != key {
            s.scan = Scan { key, ..Scan::default() };
            if s.tally_of != key.0 {
                s.tally = None;
                s.missing.clear();
                s.unloaded = 0;
            }
        }
        let end = (s.scan.index + SCAN_PER_TICK).min(volume);
        let mut i = s.scan.index;
        while i < end {
            let x = (i % size[0]) as i32;
            let z = ((i / size[0]) % size[2]) as i32;
            let y = (i / (size[0] * size[2])) as i32;
            i += 1;
            let p = [lo[0] + x, lo[1] + y, lo[2] + z];
            if !w.loaded(p) {
                if s.slots[sel].placement.wanted(p).is_some_and(|b| !b.is_air()) {
                    s.scan.unloaded += 1;
                }
                continue;
            }
            let Some((status, at, _)) = quick_status_in(w, s, sel, p) else { continue };
            let wanted = s.state_of(at).clone();
            s.scan.tally.add(status);
            if matches!(status, Status::Missing | Status::WrongBlock) {
                let per = u64::from(materials::items_for_state(&wanted));
                if per > 0
                    && let Some(item) = item_for(w, s, &wanted)
                {
                    *s.scan.missing.entry(item).or_insert(0) += per;
                }
            }
        }
        s.scan.index = i;
        if i >= volume {
            let done = std::mem::replace(&mut s.scan, Scan { key, ..Scan::default() });
            s.tally_of = key.0;
            s.tally = Some(done.tally);
            s.missing = done.missing;
            s.unloaded = done.unloaded;
        }
    });
}

// ---------------------------------------------------------------- HUD

/// What the highlights show, in words.
pub fn highlight_text(s: &State) -> String {
    let what = match (s.settings.highlight_missing, s.settings.highlight_wrong) {
        (false, false) => return "Highlight off".to_string(),
        (true, false) => "missing",
        (false, true) => "wrong",
        (true, true) => "missing and wrong",
    };
    match &s.highlight_item {
        Some(item) => format!("Highlight: {what} blocks, only {}", short(item)),
        None => format!("Highlight: {what} blocks"),
    }
}

fn inventory_counts(w: &World<'_>) -> BTreeMap<String, u64> {
    let mut m = BTreeMap::new();
    for (item, n) in w.inventory() {
        *m.entry(item).or_insert(0) += n;
    }
    m
}

/// A material's name as people write it: `oak_planks` → `Oak Planks`.
pub fn pretty_item(item: &str) -> String {
    short(item)
        .split('_')
        .map(|w| {
            let mut c = w.chars();
            c.next().map_or(String::new(), |f| f.to_uppercase().collect::<String>() + c.as_str())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Puts the selected placement's material list on the clipboard, as a table a spreadsheet takes.
fn copy_materials(w: &World<'_>) -> Result<Vec<String>, String> {
    let list = material_list(w);
    if list.is_empty() {
        return Err("no schematic loaded, or it needs no items".into());
    }
    if !w.host.set_clipboard_text(&materials::table(&list, pretty_item)) {
        return Err("the clipboard can't be written right now".into());
    }
    Ok(vec![format!("Material list copied ({} kinds): paste it into a spreadsheet", list.len())])
}

pub fn material_list(w: &World<'_>) -> Vec<Material> {
    let inv = inventory_counts(w);
    with(|s| {
        let Some(slot) = s.sel() else { return Vec::new() };
        let missing = if s.tally().is_some() { s.missing.clone() } else { slot.totals.clone() };
        materials::build_list(&slot.totals, &missing, &inv)
    })
}

pub fn volume_text(size: [i32; 3]) -> String {
    horizonmatica_core::area::volume(size).map_or_else(|| "too many".to_string(), |v| v.to_string())
}

pub fn human_size(bytes: u64) -> String {
    match bytes {
        b if b >= 1 << 30 => format!("{:.1} GB", b as f64 / f64::from(1u32 << 30)),
        b if b >= 1 << 20 => format!("{:.1} MB", b as f64 / f64::from(1u32 << 20)),
        b if b >= 1 << 10 => format!("{:.0} KB", b as f64 / 1024.0),
        b => format!("{b} B"),
    }
}

/// The info box's lines (top left) and whether the material list shows (top right).
pub fn hud_lines(w: &World<'_>) -> Vec<(String, u32)> {
    let tool = tool_held(w);
    let mut lines = with(|s| {
        let area = s.area_size();
        let Some(slot) = s.sel() else {
            return match area {
                Some(d) if s.settings.hud => {
                    vec![(format!("Horizonmatica area: {} x {} x {} (/hm save <name>)", d[0], d[1], d[2]), 0x55FFFFu32)]
                }
                _ => Vec::new(),
            };
        };
        let p = &slot.placement;
        if !s.settings.hud && !s.moving {
            return Vec::new();
        }
        let count = if s.slots.len() > 1 { format!(" ({}/{})", s.selected + 1, s.slots.len()) } else { String::new() };
        let hidden = if slot.shown { "" } else { " · hidden" };
        let mut l = vec![(format!("Horizonmatica: {}{count}{hidden}", display_name(p)), 0x55FFFFu32)];
        if s.moving {
            l.push(("MOVE MODE · wheel/arrows/PgUp/PgDn · R rotate · N done".into(), 0xFFFF55));
        }
        l.push((
            format!("Origin {} {} {} · {} · mirror {}", p.origin[0], p.origin[1], p.origin[2], p.rotation.name(), p.mirror.name()),
            0xE0E0E0,
        ));
        l.push((format!("Layers: {}{}", s.layer.describe(), if s.settings.render { "" } else { " (hidden)" }), 0xE0E0E0));
        match s.tally() {
            Some(t) => {
                l.push((format!("{:.1}% done · {} missing", t.percent(), t.missing), 0x55FF55));
                if t.wrong_block + t.wrong_state + t.extra > 0 {
                    l.push((format!("{} wrong block · {} wrong state · {} extra", t.wrong_block, t.wrong_state, t.extra), 0xFFAA00));
                }
            }
            None => l.push(("Verifying…".into(), 0xAAAAAA)),
        }
        if s.settings.printer_on {
            l.push((format!("Printer ON · {} placed", s.placed), 0xFF55FF));
            if !s.printer_note.is_empty() {
                l.push((s.printer_note.clone(), 0xAAAAAA));
            }
        }
        if s.settings.easy_place {
            l.push(("Easy place ON: only the schematic's blocks".to_string(), 0xFF55FF));
            if !s.easy_note.is_empty() {
                l.push((s.easy_note.clone(), 0xAAAAAA));
            }
        }
        if s.settings.highlight_missing || s.settings.highlight_wrong {
            l.push((highlight_text(s), 0x55FFFF));
        }
        if let Some(d) = area {
            l.push((format!("Area: {} x {} x {}", d[0], d[1], d[2]), 0xE0E0E0));
        }
        l
    });
    if let Some((save, _)) = with(|s| save_lines(w, s)) {
        lines.extend(save);
    }
    lines.extend(with(|s| load_lines(s)));
    if tool && with(|s| s.settings.tool_hud) {
        lines.extend(with(|s| tool_lines(s)));
    }
    lines
}
