//! The world-tick implementation of doll goals.
use crate::mobs::goals::{self, Cx, Goal};
use crate::mobs::{Damage, Mob, MobEvent, MobWorld, Seen, Species, Who};
use horizon_core::random::RandomSource;
use mod_alice_dolls_native::goals::*;
use mod_alice_dolls_native::stacks::DollStack as ItemStack;
use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::{Ordering};
use std::sync::Arc;

/// Whether `m` is a doll mob, named or not.
fn doll_type(m: &Mob) -> bool {
    m.kind.id == DOLL_TYPE
}

fn kind_of(m: &Mob) -> Option<Kind> {
    if !doll_type(m) {
        return None;
    }
    match m.custom_name.as_deref()? {
        SHANGHAI => Some(Kind::Shanghai),
        HORAI | DIAMOND_HORAI | NETHERITE_HORAI => Some(Kind::Horai),
        OOEDO => Some(Kind::Ooedo),
        GERMANY => Some(Kind::Germany),
        LONDON => Some(Kind::London),
        RUSSIA | MOSCOW => Some(Kind::Russia),
        GRAND_GUIGNOL => Some(Kind::GrandGuignol),
        MARISA => Some(Kind::Marisa),
        REIMU => Some(Kind::Reimu),
        AYA => Some(Kind::Aya),
        WAKASAGI => Some(Kind::Wakasagi),
        GOLIATH => Some(Kind::Goliath),
        NETHERITE_GOLIATH => Some(Kind::NetheriteGoliath),
        YUUKA => Some(Kind::Yuuka),
        YUKARI => Some(Kind::Yukari),
        PATCHOULI => Some(Kind::Patchouli),
        ALICE => Some(Kind::Alice),
        JOON => Some(Kind::Joon),
        _ => None,
    }
}

fn secret(m: &Mob) -> bool {
    in_mode(m, SECRET) || in_mode(m, SECRET_ALL)
}

/// What a hunting doll is after: diamonds and ancient debris, or in the second step every ore.
fn seeks(m: &Mob, name: &str) -> bool {
    is_treasure(name) || (in_mode(m, SECRET_ALL) && is_ore(name))
}

/// Whether a shot or blow from doll `from` must spare `who`: its owner, and every doll of the same
/// owner. Reimu's and Marisa's shots only ever fly at a target that passes this, and hit nothing else.
/// The fork's mob world shows one player, the local one, so `Who::Player` counts as the owner.
pub fn spares(from: u32, who: Who) -> bool {
    match who {
        Who::Player => true,
        Who::Mob(id) => match (state(from), state(id)) {
            (Some(a), Some(b)) => a.owner == b.owner,
            _ => false,
        },
    }
}

/// Eats the food that best fills what it has lost without wasting much: the smallest that heals it
/// fully, or else the biggest it has.
fn eat(m: &mut Mob, max: f32, love: bool) {
    let missing = max - m.health;
    let Some(items) = inventory(m.id) else { return };
    let mut foods: Vec<(Arc<str>, f32)> =
        items.into_iter().flatten().filter(|s| s.count > 0).filter_map(|s| Some((Arc::from(s.item.as_str()), food_heal(&s.item)?))).collect();
    if foods.is_empty() {
        return;
    }
    foods.sort_by(|a, b| a.1.total_cmp(&b.1));
    let (item, heal) = foods.iter().find(|(_, h)| *h >= missing).or(foods.last()).cloned().unwrap_or_default();
    if take_one(m.id, &item) {
        m.health = (m.health + heal).min(max);
        if let Ok(mut a) = ATE.lock() {
            a.push((m.id, here(m), heal, love));
        }
    }
}

fn in_mode(m: &Mob, mode: u8) -> bool {
    state(m.id).is_some_and(|s| s.enabled && s.mode == mode)
}

fn world_dimension(w: &MobWorld) -> u8 {
    dimension_code(w.level.id())
}

fn seen_of(p: &crate::mobs::PlayerView, dimension: u8) -> OwnerSeen {
    OwnerSeen {
        position: [p.position.x, p.position.y, p.position.z],
        eye: p.eye,
        velocity: [p.velocity.x, p.velocity.y, p.velocity.z],
        yaw: p.yaw,
        look: p.look,
        sneaking: p.sneaking,
        spectator: p.spectator,
        dimension,
    }
}

/// Doll `m`'s owner as it sees them: the mob world's player when that is its owner (the freshest
/// word), else where the server last saw them, if they are in this doll's level. A doll whose owner
/// isn't known yet goes by the mob world's player, as single player always has.
fn owner_seen(m: &Mob, w: &MobWorld) -> Option<OwnerSeen> {
    let owner = state(m.id).map_or(0, |s| s.owner);
    let here = world_dimension(w);
    if let Some(p) = w.player.as_ref()
        && (owner == 0 || uuid_of(p.uuid) == owner)
    {
        return Some(seen_of(p, here));
    }
    if owner == 0 {
        return None;
    }
    OWNERS.lock().ok()?.get(&owner).copied().filter(|o| o.dimension == here)
}

/// Where doll `m`'s owner stands, unless they are a spectator or out of its level.
fn owner_position(m: &Mob, cx: &Cx) -> Option<[f64; 3]> {
    owner_seen(m, cx.w).filter(|o| !o.spectator).map(|o| o.position)
}

/// Runs before every mob's tick. Sheep take the shearing a doll gave them; a doll gets its goals.
pub fn ensure(m: &mut Mob, w: &MobWorld) {
    spare_owner(m, w);
    // A hostile mob a Goliath struck turns on the Goliath (its blows are its owner's, so the mob
    // would go for the owner) while it is about, for half a minute after the last blow.
    if m.category() == "monster"
        && kind_of(m).is_none()
        && let Some(g) = goliath_foe(m.id)
        && w.seen(g).is_some_and(|s| s.alive)
    {
        m.ai.target = Some(Who::Mob(g));
    }
    if doll_type(m) {
        tick_doll(m, w);
        return;
    }
    match m.species() {
        Species::Sheep => tick_sheep(m),
        Species::Creeper | Species::Enderman | Species::ZombifiedPiglin | Species::Piglin => {}
        _ => {
            if m.category() == "monster" && !m.ai.targets.has("TargetGoliath") {
                m.ai.targets.add(4, TargetGoliath::default());
            }
        }
    }
}

fn spare_owner(m: &mut Mob, w: &MobWorld) {
    let Some(owner) = GUARDED.lock().ok().and_then(|g| g.get(&m.id).copied()) else { return };
    if !m.alive() {
        GUARDED.lock().ok().map(|mut g| g.remove(&m.id));
        return;
    }
    let spare = match m.ai.target {
        Some(Who::Mob(id)) => state(id).is_some_and(|s| s.owner == owner),
        Some(Who::Player) => {
            let at = OWNER_AT.lock().ok().and_then(|o| o.get(&owner).copied());
            match (w.player.as_ref(), at) {
                (Some(p), Some(o)) => {
                    let (dx, dy, dz) = (p.position.x - o[0], p.position.y - o[1], p.position.z - o[2]);
                    dx * dx + dy * dy + dz * dz < 4.0
                }
                _ => false,
            }
        }
        _ => false,
    };
    if spare {
        m.ai.target = None;
    }
}

/// A hostile mob with nothing to do now and then (about once in twenty seconds) picks a Goliath
/// doll within ten blocks that it can see, and gives up on it after ten seconds.
#[derive(Clone, Debug, Default)]
struct TargetGoliath {
    target: Option<u32>,
    ticks: i32,
}

impl Goal for TargetGoliath {
    fn flags(&self) -> u8 {
        goals::TARGET
    }
    fn can_use(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        if m.ai.target.is_some() || m.random.next_int_bound(400).unwrap_or(1) != 0 {
            return false;
        }
        let Ok(goliaths) = GOLIATHS.lock().map(|g| g.clone()) else { return false };
        self.target = cx
            .w
            .others
            .iter()
            .filter(|s| s.alive && goliaths.contains(&s.id) && distance_sqr(m, pos(s)) <= 100.0)
            .min_by(|a, b| distance_sqr(m, pos(a)).total_cmp(&distance_sqr(m, pos(b))))
            .map(|s| s.id)
            .filter(|&id| m.sees(cx.w, Who::Mob(id)));
        self.target.is_some()
    }
    fn can_continue(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        self.ticks < 200
            && m.ai.target == self.target.map(Who::Mob)
            && self.target.and_then(|id| cx.w.seen(id)).is_some_and(|s| s.alive && distance_sqr(m, pos(s)) <= 576.0)
    }
    fn start(&mut self, m: &mut Mob, _: &mut Cx) {
        self.ticks = 0;
        m.ai.target = self.target.map(Who::Mob);
    }
    fn tick(&mut self, _: &mut Mob, _: &mut Cx) {
        self.ticks += 1;
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn stop(&mut self, m: &mut Mob, _: &mut Cx) {
        if m.ai.target == self.target.map(Who::Mob) {
            m.ai.target = None;
        }
        self.target = None;
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
    fn name(&self) -> &'static str {
        "TargetGoliath"
    }
}

/// Rideon: on the owner's head every tick, before the doll moves, ahead by the owner's own motion
/// so it does not trail behind.
fn sit_on_head(m: &mut Mob, w: &MobWorld) {
    let Some(p) = owner_seen(m, w).filter(|o| !o.spectator) else { return };
    m.position.x = p.position[0] + p.velocity[0];
    m.position.y = p.eye[1] + 0.25 + p.velocity[1];
    m.position.z = p.position[2] + p.velocity[2];
    m.previous = m.position;
    m.velocity = horizon_core::geometry::Vec3::new(0.0, 0.0, 0.0);
    m.fall_distance = 0.0;
    m.yaw = p.yaw;
    m.nav_stop();
}

fn tick_sheep(m: &mut Mob) {
    if let Ok(mut sheared) = SHEARED.lock()
        && let Some(at) = sheared.iter().position(|&id| id == m.id)
    {
        sheared.swap_remove(at);
        m.extra.sheared = true;
    }
    if let Ok(mut shearable) = SHEARABLE.lock() {
        if !m.extra.sheared && !m.is_baby() && m.alive() {
            shearable.insert(m.id);
        } else {
            shearable.remove(&m.id);
        }
    }
}

/// A doll mob named as a doll, whether or not the api crate has spoken for it yet: the sound hooks
/// ask this, so a doll is quiet from its first tick, on a client too.
pub fn is_doll_mob(m: &Mob) -> bool {
    kind_of(m).is_some()
}

/// How high Aya floats over the ground, and how far down she looks for it. Under a block: a path's
/// waypoint counts as reached only within a block of height.
const AYA_HOVER: f64 = 0.85;
const AYA_GROUND_LOOK: i32 = 4;

/// Aya in flight: no gravity, and a gentle climb or sink to `AYA_HOVER` over the ground under her.
/// Over a drop deeper than she looks she glides down slowly. On her owner's head she rides.
fn hover(m: &mut Mob, w: &MobWorld) {
    if in_mode(m, RIDE) {
        m.no_gravity = false;
        return;
    }
    m.no_gravity = true;
    let (x, z) = (m.position.x.floor() as i32, m.position.z.floor() as i32);
    let y0 = m.position.y.floor() as i32;
    let ground = (0..=AYA_GROUND_LOOK)
        .map(|d| y0 - d)
        .find(|&y| w.facts.get(w.world.block(x, y - 1, z)).full_collision)
        .map(f64::from);
    m.velocity.y = match ground {
        Some(g) => ((g + AYA_HOVER - m.position.y) * 0.3).clamp(-0.15, 0.2),
        None => -0.08,
    };
    // Over ground she counts as standing on it: walking paths are only made and followed by a mob
    // on the ground, and she moves at her walking speed instead of the slow speed in the air.
    if ground.is_some() {
        m.on_ground = true;
    }
}

fn tick_doll(m: &mut Mob, w: &MobWorld) {
    let Some(kind) = kind_of(m) else { return };
    // Silent stops the hurt, death and step sounds; the doll-voice hook stops the rest.
    m.silent = true;
    // A doll falls slowly, flapping its skirt (the chicken's `aiStep`), and takes no fall damage.
    if !m.on_ground && m.velocity.y < 0.0 && !m.no_gravity {
        m.velocity.y *= 0.6;
    }
    m.fall_distance = 0.0;
    // Aya flies: she floats about a block off the ground, wherever she goes.
    if kind == Kind::Aya {
        hover(m, w);
    }
    // Checked every tick: the game may hand the mob its type's sounds back.
    if !m.kind.sounds[0].is_empty() {
        let kind = Arc::make_mut(&mut m.kind);
        kind.sounds = ["", "", ""];
    }
    // Every walk and run of a doll scales from this.
    m.attrs.speed = m.kind.speed * SPEED_SCALE * speed_factor(kind);
    // A doll's health is its own, for its love level. It is set as the attribute's base, not only
    // the cached maximum (which anything that reads the attribute again puts back to the mob type's),
    // and only where the doll's state, and so its level, is known: nothing that hasn't been told its
    // level heals it back and forth. A doll whose maximum changes gets its new health in full.
    let health = levelled_health(max_health(kind), level_of(m.id));
    if state(m.id).is_some() {
        use crate::mobs::finalize::{Attr, base_value, set_base};
        if (base_value(m, Attr::MaxHealth) - f64::from(health)).abs() > 1e-3 {
            set_base(m, Attr::MaxHealth, f64::from(health));
            if m.alive() {
                m.health = health;
            }
        }
    }
    // Whether the player is sneaking, for the api crate's shift-click pickup.
    if let Some(p) = w.player.as_ref() {
        SNEAKING.store(p.sneaking, Ordering::Relaxed);
    }
    // The Netherite Horai is a third quicker than the others.
    if horai_tier(m) == 2 {
        m.attrs.speed *= 1.3;
    }
    let Some(state) = state(m.id) else { return };
    if m.alive() && m.health < health * RESCUE_SHARE {
        in_distress(m, w, state.owner);
    }
    // Hurt, it eats from its own inventory: once every two seconds, healing what the food fills.
    // Whole again: what its owner did is forgotten.
    if m.health >= health && OWNER_HURT.lock().is_ok_and(|mut h| h.remove(&m.id)) {}
    // Healing what its owner did to it earns no love; healing a mob's or another player's blows does.
    // Joon's inventory is her stock for trading: she eats none of it.
    let joon = kind == Kind::Joon;
    if !joon && m.alive() && m.health <= health - 1.0 && m.age % 40 == 0 {
        eat(m, health, !hurt_by_owner(m.id));
    }
    // At work (on patrol, following or on the head), it eats a piece of food every five minutes,
    // hungry or not.
    if !joon && m.alive() && matches!(state.mode, PATROL | FOLLOW | RIDE) && m.age > 0 && m.age % MEAL_TICKS == 0 {
        eat(m, health, true);
    }
    if kind == Kind::Wakasagi && m.fluids.in_water && state.mode != RIDE {
        swim(m);
    }
    // Every doll wears what is in its armour slots; Joon has none, only stock.
    if !joon {
        wear_armour(m);
    }
    // The nimble fabric: a shot it was about to take, it teleports away from.
    if DODGE.lock().is_ok_and(|mut d| d.remove(&m.id)) {
        dodge(m);
    }
    if matches!(kind, Kind::Goliath | Kind::NetheriteGoliath) {
        // Regeneration: a point of health a second.
        if m.alive() && m.health < health && m.age % 20 == 0 {
            m.health = (m.health + 1.0).min(health);
        }
        if let Ok(mut g) = GOLIATHS.lock() {
            g.insert(m.id);
        }
    }
    if kind == Kind::Alice
        && let Ok(mut i) = IDLE.lock()
    {
        i.insert(m.id);
    }
    m.no_gravity = state.mode == RIDE;
    if state.mode == RIDE && state.enabled {
        sit_on_head(m, w);
    }
    // Its jobs are set once for each form: a doll that evolves (a Diamond Horai into a Netherite
    // one, a Goliath into a Netherite one) gets its new form's jobs, the quarry among them.
    let form = m.custom_name.clone().unwrap_or_default();
    if m.ai.goals.has("DollFollow") && GOALS_FORM.lock().is_ok_and(|f| f.get(&m.id) == Some(&form)) {
        return;
    }
    if let Ok(mut f) = GOALS_FORM.lock() {
        f.insert(m.id, form);
    }
    let tier = horai_tier(m);
    let ai = &mut m.ai;
    ai.goals.clear();
    ai.targets.clear();
    ai.goals.add(0, goals::Float);
    ai.goals.add(0, DollRide);
    // Each doll's own jobs: its `onInitializeAI`.
    match kind {
        Kind::Shanghai => {
            ai.goals.add(1, DollReport::default());
            ai.goals.add(2, DollAttack::default());
            ai.goals.add(3, DollSteal::default());
        }
        Kind::Horai => {
            ai.goals.add(1, ReportOre::default());
            if tier >= 1 {
                ai.goals.add(1, StripMine::default());
                ai.goals.add(1, SeekOre::default());
            }
            if tier == 2 {
                ai.goals.add(1, Quarry::default());
            }
            ai.goals.add(1, GigaQuarry::default());
            ai.goals.add(1, FetchDrops::default());
            ai.goals.add(2, MineOre::default());
        }
        Kind::Ooedo => {
            ai.goals.add(0, Detonate::default());
            ai.goals.add(4, Explode::default());
        }
        // She plants a sapling from her inventory where each tree she fells stood. Ahead of
        // picking up what is dropped (5): at 10, every log it cut drew it off the tree to fetch the
        // drop, and the tree was forgotten; now it fells the whole tree, then gathers.
        Kind::Germany => {
            ai.goals.add(2, CutTree::default());
        }
        Kind::London => {
            ai.goals.add(11, ReportSpawners::default());
        }
        Kind::Russia => {
            ai.goals.add(5, CollectItem::default());
            ai.goals.add(6, Torcher::default());
        }
        Kind::Marisa => {
            ai.goals.add(1, Danmaku::new(Shot::Beam));
        }
        Kind::Reimu => {
            ai.goals.add(1, Danmaku::new(Shot::Amulet));
        }
        Kind::Aya => {
            ai.goals.add(1, AyaShot::default());
        }
        Kind::Goliath => {
            ai.goals.add(1, GoliathAttack::default());
        }
        Kind::NetheriteGoliath => {
            ai.goals.add(1, GoliathAttack { strength: GOLIATH_STRENGTH + 2.0, ..GoliathAttack::default() });
        }
        Kind::Yuuka => {
            ai.goals.add(1, Farmer::default());
        }
        Kind::Wakasagi => {
            ai.goals.add(2, WakasagiHook::default());
            ai.goals.add(5, Fishing::default());
        }
        Kind::GrandGuignol => {
            ai.goals.add(-10, Mission);
        }
        // Yukari follows her owner (her one mode) as their portable gap, and leaves mobs alone.
        Kind::Yukari => {}
        // Patchouli snipes: one slow, heavy orb at a time, from far off.
        Kind::Patchouli => {
            ai.goals.add(1, Snipe::default());
        }
        // Alice does nothing yet but go where her mode has her.
        Kind::Alice => {}
        // Joon follows whoever she is with (her owner, or the player she is visiting) and trades.
        Kind::Joon => {}
    }
    let idle = kind == Kind::Alice;
    // Alice, Yukari and Joon attack nothing at all (nor pick anything up).
    let peaceful = idle || kind == Kind::Yukari || kind == Kind::Joon;
    // The dolls that don't fight still stand up for a badly hurt doll near them.
    if !fights(kind) && !peaceful {
        ai.goals.add(0, DollAttack { defend_only: true, ..DollAttack::default() });
    }
    // Yukari doesn't go for dropped items either.
    if kind != Kind::Russia && !peaceful {
        ai.goals.add(5, CollectItem::default());
    }
    ai.goals.add(4, MoscowFollow { recalculate_in: 0 });
    ai.goals.add(1, AnchorLeash { recalculate_in: 0, walked: 0 });
    ai.goals.add(12, AnchorHome { recalculate_in: 0, walked: 0 });
    if !peaceful {
        ai.goals.add(0, FoxHunt::default());
    }
    ai.goals.add(12, DollFollow { recalculate_in: 0 });
    ai.goals.add(13, goals::Stroll::new(1.0).gate(patrolling).named("DollWander"));
    ai.goals.add(14, goals::LookAtPlayer::new(8.0));
    ai.goals.add(16, goals::RandomLookAround::default());
}

/// Wakasagi, a mermaid, swims as fast as a dolphin wherever her path leads in water: a walking mob
/// only paddles there, whatever its speed.
fn swim(m: &mut Mob) {
    if m.ai.nav.path.is_none() {
        return;
    }
    let yaw = f64::from(m.yaw).to_radians();
    let (dx, dz) = (-yaw.sin(), yaw.cos());
    m.velocity.x = m.velocity.x * 0.4 + dx * SWIM_SPEED * 0.6;
    m.velocity.z = m.velocity.z * 0.4 + dz * SWIM_SPEED * 0.6;
}

/// The original mod's armour (`EntityAliceDoll.markDirty`): the last four slots of a doll's
/// inventory are what it wears, the very last its helmet, then its chestplate, leggings and boots,
/// each only when it is that piece; its armour then counts against every blow. A shield anywhere in
/// its inventory goes in its off hand. What it wears stays in its inventory, never dropped as the
/// mob's equipment.
fn wear_armour(m: &mut Mob) {
    use crate::mobs::finalize::{self, CHEST, FEET, HEAD, LEGS};
    let items = inventory(m.id).unwrap_or_default();
    let n = items.len();
    let mut want: [Option<ItemStack>; 4] = Default::default();
    if n >= ARMOUR_SLOTS {
        for (part, slot) in want.iter_mut().enumerate() {
            if let Some(s) = items[armour_slot(n, part)].clone()
                && armour_part(&s.item) == Some(part)
            {
                *slot = Some(s);
            }
        }
    }
    for (part, slot) in [HEAD, CHEST, LEGS, FEET].into_iter().enumerate() {
        if m.extra.slot_id(slot) == want[part].as_ref().map(|stack| stack.item.as_str()) {
            continue;
        }
        match want[part].take() {
            Some(stack) => {
                let Some(stack) = crate::items::ItemStack::from_nbt(&stack.encode()) else {
                    continue;
                };
                finalize::set_item_stack(m, slot, stack);
                finalize::set_drop_chance(m, slot, 0.0);
            }
            None => m.extra.set_slot_id(slot, None),
        }
    }
    let shield = items.iter().flatten().any(|s| &*s.item == SHIELD);
    if shield != (m.extra.slot_id(finalize::OFFHAND) == Some(SHIELD)) {
        m.extra.set_slot_id(finalize::OFFHAND, shield.then(|| Arc::from(SHIELD)));
        finalize::set_drop_chance(m, finalize::OFFHAND, 0.0);
    }
}

fn patrolling(m: &Mob, _: &MobWorld) -> bool {
    in_mode(m, PATROL)
}


fn distance_sqr(m: &Mob, at: [f64; 3]) -> f64 {
    let (dx, dy, dz) = (m.position.x - at[0], m.position.y - at[1], m.position.z - at[2]);
    dx * dx + dy * dy + dz * dz
}

fn pos(s: &Seen) -> [f64; 3] {
    [s.position.x, s.position.y, s.position.z]
}

/// The box the original searched: `boundingBox.expand(range, height, range)`.
fn noticed(from: [f64; 3], s: &Seen) -> bool {
    s.alive
        && (s.position.x - from[0]).abs() <= SEARCH_RANGE
        && (s.position.y - from[1]).abs() <= SEARCH_HEIGHT
        && (s.position.z - from[2]).abs() <= SEARCH_RANGE
}

fn here(m: &Mob) -> [f64; 3] {
    [m.position.x, m.position.y, m.position.z]
}

/// The nearest noticed mob that passes `wanted`.
fn nearest(m: &Mob, cx: &Cx, wanted: impl Fn(&Seen) -> bool) -> Option<u32> {
    let from = here(m);
    cx.w.others
        .iter()
        .filter(|s| s.id != m.id && noticed(from, s) && wanted(s))
        .min_by(|a, b| distance_sqr(m, pos(a)).total_cmp(&distance_sqr(m, pos(b))))
        .map(|s| s.id)
}

/// What a patrolling doll attacks: every hostile mob (the `monster` category, plus the hostile mobs
/// filed elsewhere), as the original's `targetEntityRegex` named the hostile mobs of 1.7. Zombified
/// piglins and piglins are left alone: hitting one turns the whole group on the owner.
fn patrol_target(s: &Seen) -> bool {
    if matches!(s.species, Species::ZombifiedPiglin | Species::Piglin) {
        return false;
    }
    s.category == "monster"
        || matches!(
            s.species,
            Species::Slime
                | Species::MagmaCube
                | Species::Ghast
                | Species::Phantom
                | Species::Shulker
                | Species::Hoglin
                | Species::Zoglin
                | Species::PiglinBrute
                | Species::Guardian
                | Species::ElderGuardian
                | Species::Wither
                | Species::ZombieNautilus
        )
}

/// Rideon mode: sits on the player's head.
#[derive(Clone, Debug)]
struct DollRide;

impl Goal for DollRide {
    // Only JUMP: the hook before each tick holds the doll on the head, and its jobs (Horai's mining,
    // Shanghai's fighting) still need MOVE and LOOK while it rides.
    fn flags(&self) -> u8 {
        goals::JUMP
    }
    fn can_use(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        in_mode(m, RIDE) && owner_position(m, cx).is_some()
    }
    fn start(&mut self, m: &mut Mob, _: &mut Cx) {
        m.nav_stop();
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        // Holds MOVE so nothing else walks it off; the hook before each tick does the sitting.
        sit_on_head(m, cx.w);
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
    fn name(&self) -> &'static str {
        "DollRide"
    }
}

/// `EntityDollAIReportEntity`: every second, what it can see, in any mode.
#[derive(Clone, Debug, Default)]
struct DollReport {
    last: i64,
}

impl Goal for DollReport {
    fn flags(&self) -> u8 {
        0
    }
    fn can_use(&mut self, m: &mut Mob, _: &mut Cx) -> bool {
        state(m.id).is_some_and(|s| s.enabled)
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        if cx.w.game_time - self.last < 20 {
            return;
        }
        self.last = cx.w.game_time;
        let from = here(m);
        let mut counts: BTreeMap<String, u32> = BTreeMap::new();
        for s in cx.w.others.iter().filter(|s| s.id != m.id && noticed(from, s) && !is_doll(s.id)) {
            *counts.entry(format!("{:?}", s.species)).or_default() += 1;
        }
        if counts.is_empty() {
            say(m.id, 2, "No target");
        } else {
            let list: String = counts.iter().map(|(name, n)| format!("{name}[{n}]")).collect();
            say(m.id, 1, list);
        }
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
    fn name(&self) -> &'static str {
        "DollReport"
    }
}

/// Who doll `m` holds a grudge against: whoever hurt it, else whoever hurt its owner, in the last
/// half minute, and whoever is after a badly hurt doll near it. Never its owner, nor one of its
/// owner's dolls.
fn grudge(m: &Mob) -> Vec<Foe> {
    let mut out = grudges_of(m.id, state(m.id).map_or(0, |s| s.owner));
    for f in distress_foes(m) {
        if !out.contains(&f) {
            out.push(f);
        }
    }
    out
}

/// Doll `m`, owned by `owner`, is badly hurt: whoever hurt it lately is every doll's foe about it,
/// and with its owner away, the nearest of its owner's dolls picks it up (once a second).
fn in_distress(m: &Mob, w: &MobWorld, owner: u128) {
    let at = here(m);
    let foe = DOLL_GRUDGES.lock().ok().and_then(|g| g.get(&m.id).filter(|(_, t)| t.elapsed() < GRUDGE).map(|(f, _)| *f));
    if let (Some(foe), Ok(mut d)) = (foe, DISTRESS.lock()) {
        d.retain(|_, (_, _, t)| t.elapsed() < GRUDGE);
        d.insert(m.id, (foe, at, std::time::Instant::now()));
    }
    if m.age % 20 != 0 || owner == 0 || riding(m.id) {
        return;
    }
    let dim = world_dimension(w);
    let owner_near = OWNERS.lock().is_ok_and(|o| {
        o.get(&owner).is_some_and(|p| {
            let d2: f64 = (0..3).map(|i| (p.position[i] - at[i]).powi(2)).sum();
            p.dimension == dim && d2 <= OWNER_NEAR * OWNER_NEAR
        })
    });
    if owner_near {
        return;
    }
    let rescuer = w
        .others
        .iter()
        .filter(|s| s.alive && s.id != m.id && !riding(s.id) && !idle(s.id) && state(s.id).is_some_and(|t| t.owner == owner))
        .filter(|s| distance_sqr(m, pos(s)) <= RESCUE_RANGE * RESCUE_RANGE)
        .min_by(|a, b| distance_sqr(m, pos(a)).total_cmp(&distance_sqr(m, pos(b))))
        .map(|s| s.id);
    if let (Some(r), Ok(mut q)) = (rescuer, RESCUES.lock())
        && !q.iter().any(|(h, _)| *h == m.id)
    {
        q.push((m.id, r));
    }
}

/// Whoever is after a badly hurt doll within reach of doll `m`: never `m`'s owner, nor a doll of
/// its owner's.
fn distress_foes(m: &Mob) -> Vec<Foe> {
    let owner = state(m.id).map_or(0, |s| s.owner);
    let Ok(d) = DISTRESS.lock() else { return Vec::new() };
    let mut out: Vec<Foe> = d
        .iter()
        .filter(|(id, (_, at, t))| **id != m.id && t.elapsed() < GRUDGE && distance_sqr(m, *at) <= DEFEND_RANGE * DEFEND_RANGE)
        .map(|(_, (f, _, _))| *f)
        .filter(|f| match *f {
            Foe::Player(p) => p != owner,
            Foe::Mob(other) => other != m.id && !spares(m.id, Who::Mob(other)),
        })
        .collect();
    out.dedup();
    out
}

/// Whether mob `s` is attacking doll `m`: it has `m` as its target, or hurt it in the last half minute.
fn attacking_me(m: &Mob, s: &Seen) -> bool {
    s.alive
        && !is_doll_of_same_owner(m, s)
        && (matches!(s.target, Some(Who::Mob(id)) if id == m.id)
            || DOLL_GRUDGES
                .lock()
                .ok()
                .and_then(|g| g.get(&m.id).copied())
                .is_some_and(|(f, t)| f == Foe::Mob(s.id) && t.elapsed() < GRUDGE))
}

/// The nearest mob attacking doll `m`: a fighting doll goes for it before anything else.
fn my_attacker(m: &Mob, cx: &Cx) -> Option<u32> {
    nearest(m, cx, |s| attacking_me(m, s))
}

/// Whether a fighting doll should drop `target` for a mob attacking it (looked at twice a second).
fn attacker_elsewhere(m: &Mob, cx: &Cx, target: Option<u32>) -> bool {
    cx.w.game_time % 10 == 0
        && !target.and_then(|t| cx.w.seen(t)).is_some_and(|s| attacking_me(m, s))
        && my_attacker(m, cx).is_some()
}

/// Whether doll `m` fights back at mob `s`: it has `m` as its target, or hurt `m` or its owner lately.
fn fights_back(m: &Mob, s: &Seen) -> bool {
    !is_doll_of_same_owner(m, s)
        && (matches!(s.target, Some(Who::Mob(id)) if id == m.id) || grudge(m).contains(&Foe::Mob(s.id)))
}

fn is_doll_of_same_owner(m: &Mob, s: &Seen) -> bool {
    spares(m.id, Who::Mob(s.id))
}

/// The player doll `m` goes after, if any: one who hurt it or its owner lately, in its level, not a
/// spectator, and within the distance it notices things at. Their UUID and where they stand.
fn player_foe(m: &Mob, w: &MobWorld) -> Option<(u128, OwnerSeen)> {
    player_foe_among(m, w, grudge(m))
}

/// [`player_foe`] among `foes`.
fn player_foe_among(m: &Mob, w: &MobWorld, foes: Vec<Foe>) -> Option<(u128, OwnerSeen)> {
    let here_dim = world_dimension(w);
    let from = here(m);
    let players = OWNERS.lock().ok()?;
    foes.into_iter().find_map(|f| {
        let Foe::Player(p) = f else { return None };
        let o = players.get(&p).copied()?;
        let close = (o.position[0] - from[0]).abs() <= SEARCH_RANGE
            && (o.position[1] - from[1]).abs() <= SEARCH_HEIGHT
            && (o.position[2] - from[2]).abs() <= SEARCH_RANGE;
        (o.dimension == here_dim && !o.spectator && close).then_some((p, o))
    })
}

/// Whether a doll in mode `m` fights at all: on patrol, following, or riding.
fn fighting_mode(m: &Mob) -> bool {
    in_mode(m, PATROL) || in_mode(m, FOLLOW) || in_mode(m, RIDE)
}

/// `EntityDollAIAttackEnemy`, made steadier: on patrol, every hostile mob around it; following,
/// every hostile mob near its owner as well as whatever is after its owner. In any of those modes it
/// also fights back at a mob that targets it or hurt it or its owner, and at a player who hurt it or
/// its owner in the last half minute (players only when no mob is to be fought). It looks again twice a
/// second, re-paths four times a second, and strikes as often as a stone sword can be swung.
#[derive(Clone, Debug, Default)]
struct DollAttack {
    /// Only for a badly hurt doll near it: the dolls that don't fight otherwise.
    defend_only: bool,
    target: Option<u32>,
    /// A player it goes after, when there is no mob to fight.
    player: Option<u128>,
    last_scan: i64,
    last_hit: i64,
    last_path: i64,
    lost: i32,
}

fn near_owner(m: &Mob, cx: &Cx, s: &Seen) -> bool {
    owner_position(m, cx).is_some_and(|p| {
        let (dx, dy, dz) = (s.position.x - p[0], s.position.y - p[1], s.position.z - p[2]);
        dx * dx + dy * dy + dz * dz <= OWNER_GUARD * OWNER_GUARD
    })
}

fn enemy(s: &Seen) -> bool {
    (patrol_target(s) || touhou_attacker(s) || angry_golem(s)) && !off_limits(s)
}

/// What no doll ever shoots or strikes: any doll (known to this side or not: one far off, or whose
/// first tick hasn't said whose it is yet, is a doll by its type), a tamed or owned animal, and a
/// mount such as a broom.
fn off_limits(s: &Seen) -> bool {
    is_doll(s.id) || s.kind.id == DOLL_TYPE || s.tame || s.owner.is_some() || MOUNTS.lock().is_ok_and(|m| m.contains(&s.id))
}

/// An iron golem that has turned on the player: fighting dolls go for it like any hostile mob.
fn angry_golem(s: &Seen) -> bool {
    s.species == Species::IronGolem && matches!(s.target, Some(Who::Player))
}

/// A Touhou Mobs mob that is attacking: Touhou Mobs builds them on the wandering trader, and one
/// holds the player as its target only while it fights (a wandering trader never targets anyone).
fn touhou_attacker(s: &Seen) -> bool {
    s.species == Species::WanderingTrader && matches!(s.target, Some(Who::Player))
}

/// Close enough to steal: two and a half blocks across, and up to three above or below.
fn in_reach(m: &Mob, at: [f64; 3]) -> bool {
    within(m, at, REACH_SQR, 3.0)
}

/// Close enough to strike: seven blocks across, and up to seven above or below, so a big mob the
/// path stops short of, a low-flying fairy or phantom, or a mob below the owner's head can be hit.
fn in_strike_reach(m: &Mob, at: [f64; 3]) -> bool {
    within(m, at, ATTACK_REACH_SQR, 7.0)
}

fn within(m: &Mob, at: [f64; 3], across_sqr: f64, up: f64) -> bool {
    let (dx, dy, dz) = (m.position.x - at[0], m.position.y - at[1], m.position.z - at[2]);
    dx * dx + dz * dz < across_sqr && dy.abs() < up
}

impl Goal for DollAttack {
    fn flags(&self) -> u8 {
        goals::MOVE | goals::LOOK
    }
    fn can_use(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        if cx.w.game_time - self.last_scan < SCAN_EVERY {
            return false;
        }
        self.last_scan = cx.w.game_time;
        if self.defend_only {
            if !fighting_mode(m) {
                return false;
            }
            let foes = distress_foes(m);
            if foes.is_empty() {
                return false;
            }
            self.target = nearest(m, cx, |s| foes.contains(&Foe::Mob(s.id)));
            self.player = if self.target.is_none() { player_foe_among(m, cx.w, foes).map(|(p, _)| p) } else { None };
            return self.target.is_some() || self.player.is_some();
        }
        // Whatever is attacking it comes first.
        self.target = if let Some(a) = fighting_mode(m).then(|| my_attacker(m, cx)).flatten() {
            Some(a)
        } else if in_mode(m, PATROL) {
            nearest(m, cx, |s| enemy(s) || fights_back(m, s))
        } else if in_mode(m, FOLLOW) || in_mode(m, RIDE) {
            nearest(m, cx, |s| {
                matches!(s.target, Some(Who::Player)) || (enemy(s) && near_owner(m, cx, s)) || fights_back(m, s)
            })
        } else {
            None
        };
        self.player = if self.target.is_none() && fighting_mode(m) { player_foe(m, cx.w).map(|(p, _)| p) } else { None };
        self.target.is_some() || self.player.is_some()
    }
    fn can_continue(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        if !self.defend_only && fighting_mode(m) && attacker_elsewhere(m, cx, self.target) {
            return false;
        }
        if let Some(p) = self.player {
            return fighting_mode(m) && self.lost < LOST_AFTER && player_foe(m, cx.w).is_some_and(|(q, _)| q == p);
        }
        let Some(s) = self.target.and_then(|id| cx.w.seen(id)) else { return false };
        let keep = if self.defend_only {
            fighting_mode(m) && distress_foes(m).contains(&Foe::Mob(s.id))
        } else if in_mode(m, PATROL) {
            true
        } else if in_mode(m, FOLLOW) || in_mode(m, RIDE) {
            // A following doll does not chase off: it lets go of anything that leaves its owner,
            // unless it is fighting back.
            matches!(s.target, Some(Who::Player)) || near_owner(m, cx, s) || fights_back(m, s)
        } else {
            false
        };
        keep && s.alive && self.lost < LOST_AFTER
    }
    fn start(&mut self, _: &mut Mob, _: &mut Cx) {
        self.lost = 0;
        self.last_path = 0;
    }
    fn stop(&mut self, m: &mut Mob, _: &mut Cx) {
        self.target = None;
        self.player = None;
        m.nav_stop();
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        if let Some(p) = self.player {
            let Some((_, o)) = player_foe(m, cx.w).filter(|(q, _)| *q == p) else {
                self.lost += 1;
                return;
            };
            m.look_at(o.eye);
            if !in_mode(m, RIDE) && cx.w.game_time - self.last_path >= PATH_EVERY {
                self.last_path = cx.w.game_time;
                m.nav_move_to_entity(cx.w, o.position, 0, 1.0);
            }
            if in_strike_reach(m, o.position) && cx.w.game_time - self.last_hit >= HIT_EVERY {
                self.last_hit = cx.w.game_time;
                m.swing();
                mark_swing(m.id);
                hit_player(m.id, p, attack(m.id, ATTACK_STRENGTH));
                worked(m.id);
            }
            return;
        }
        let Some(id) = self.target else { return };
        let Some((at, eye, alive)) = cx.w.seen(id).map(|s| (pos(s), s.eye_y, s.alive)) else {
            return;
        };
        m.look_at([at[0], eye, at[2]]);
        if m.sees(cx.w, Who::Mob(id)) {
            self.lost = 0;
        } else {
            self.lost += 1;
        }
        // A riding doll strikes from the head, at whatever comes within reach.
        if !in_mode(m, RIDE) && cx.w.game_time - self.last_path >= PATH_EVERY {
            self.last_path = cx.w.game_time;
            m.nav_move_to_entity(cx.w, at, 0, 1.0);
        }
        if alive && in_strike_reach(m, at) && cx.w.game_time - self.last_hit >= HIT_EVERY {
            self.last_hit = cx.w.game_time;
            m.swing();
            mark_swing(m.id);
            // DamageSource.causePlayerDamage(owner): the kill and its experience are the owner's.
            cx.events.push(MobEvent::AttackMob {
                target: id,
                amount: attack(m.id, ATTACK_STRENGTH),
                source: Damage::player(here(m)),
            });
            worked(m.id);
        }
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
    fn name(&self) -> &'static str {
        "DollAttack"
    }
}

/// Every doll the moment it sees a fox. On patrol or following it teleports beside it; on standby or
/// riding its owner's head it strikes from where it is, up to `FOX_RANGE` away. It strikes
/// it for nine tenths of its health and sends it far up into the sky. It watches the fox down, and
/// one that lands alive anyway (in water, say) it finishes.
#[derive(Clone, Debug, Default)]
struct FoxHunt {
    fox: Option<u32>,
    /// Struck from where it is (standby, or on the head), not teleported to.
    ranged: bool,
    /// The highest the fox has been since it went up.
    peak: f64,
    launched_at: f64,
    ticks: i32,
}

impl Goal for FoxHunt {
    fn flags(&self) -> u8 {
        goals::MOVE | goals::LOOK
    }
    fn can_use(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        if let Ok(mut f) = FOXES.lock() {
            f.extend(cx.w.others.iter().filter(|s| s.species == Species::Fox).map(|s| s.id));
            if f.len() > 4096 {
                f.clear();
            }
        }
        self.ranged = in_mode(m, STANDBY) || in_mode(m, RIDE);
        let found = if self.ranged {
            let from = here(m);
            cx.w.others
                .iter()
                .filter(|s| s.species == Species::Fox && s.alive && !launched(s.id) && distance_sqr(m, pos(s)) <= FOX_RANGE * FOX_RANGE)
                .min_by(|a, b| distance_sqr(m, pos(a)).total_cmp(&distance_sqr(m, pos(b))))
                .map(|s| s.id)
                .filter(|_| from[1].is_finite())
        } else {
            nearest(m, cx, |s| s.species == Species::Fox && !launched(s.id))
        };
        let Some(id) = found else { return false };
        if !m.sees(cx.w, Who::Mob(id)) {
            return false;
        }
        self.fox = Some(id);
        true
    }
    fn can_continue(&mut self, _: &mut Mob, cx: &mut Cx) -> bool {
        self.fox.and_then(|id| cx.w.seen(id)).is_some_and(|s| s.alive) && self.ticks < 400
    }
    fn start(&mut self, m: &mut Mob, cx: &mut Cx) {
        let Some(id) = self.fox else { return };
        let Some(s) = cx.w.seen(id) else { return };
        let at = pos(s);
        if !self.ranged {
            // Beside it, between it and the doll.
            let (dx, dz) = (m.position.x - at[0], m.position.z - at[2]);
            let d = (dx * dx + dz * dz).sqrt().max(1e-6);
            m.position.x = at[0] + dx / d * 0.8;
            m.position.y = at[1];
            m.position.z = at[2] + dz / d * 0.8;
            m.previous = m.position;
            m.nav_stop();
            cx.events.push(MobEvent::Sound("entity.enderman.teleport", here(m), 1.0, 1.0));
        }
        m.look_at([at[0], s.eye_y, at[2]]);
        m.swing();
        mark_swing(m.id);
        cx.events.push(MobEvent::AttackMob { target: id, amount: FOX_HEALTH * 0.9, source: Damage::player(here(m)) });
        cx.events.push(MobEvent::Push { target: Who::Mob(id), velocity: [0.0, FOX_LAUNCH, 0.0] });
        cx.events.push(MobEvent::Sound("entity.player.attack.knockback", at, 1.0, 1.0));
        if let Ok(mut l) = LAUNCHED.lock() {
            l.insert(id);
        }
        self.peak = at[1];
        self.launched_at = at[1];
        self.ticks = 0;
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        self.ticks += 1;
        let Some(id) = self.fox else { return };
        let Some((y, vy, alive)) = cx.w.seen(id).map(|s| (s.position.y, s.velocity.y, s.alive)) else { return };
        self.peak = self.peak.max(y);
        // Down again and stopped, alive: the fall did not do it, so the doll does.
        let landed = self.ticks > 20 && self.peak > self.launched_at + 10.0 && vy.abs() < 0.05 && y < self.peak - 5.0;
        if alive && landed {
            cx.events.push(MobEvent::AttackMob { target: id, amount: 1000.0, source: Damage::player(here(m)) });
            self.ticks = 400;
        }
    }
    fn stop(&mut self, _: &mut Mob, _: &mut Cx) {
        if let (Some(id), Ok(mut l)) = (self.fox.take(), LAUNCHED.lock()) {
            l.remove(&id);
        }
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
    fn name(&self) -> &'static str {
        "DollFoxHunt"
    }
}

/// `EntityDollAIStealItem`: on patrol, shears sheep, and now and then plucks a chicken, picks a
/// mooshroom's mushroom or takes the rose an iron golem is holding out.
#[derive(Clone, Debug, Default)]
struct DollSteal {
    target: Option<u32>,
    last_scan: i64,
    last_try: i64,
    lost: i32,
    rose_offered: bool,
}

impl DollSteal {
    /// What to do on reaching `target`: true when finished with it.
    fn take(&mut self, m: &mut Mob, cx: &mut Cx, target: u32, species: Species, color: u8, at: [f64; 3]) -> bool {
        let drop = |cx: &mut Cx, item: &str, count: u32| {
            cx.events.push(MobEvent::DropItem { mob: target, item: Arc::from(item), count, at });
        };
        match species {
            Species::Sheep => {
                if let Ok(mut sheared) = SHEARED.lock() {
                    sheared.push(target);
                }
                if let Ok(mut shearable) = SHEARABLE.lock() {
                    shearable.remove(&target);
                }
                let count = 1 + m.random.next_int_bound(3).unwrap_or(0) as u32;
                drop(cx, WOOL[usize::from(color) % WOOL.len()], count);
                cx.events.push(MobEvent::Sound("entity.sheep.shear", at, 0.5, 1.0));
                say(m.id, 2, "Wool!");
                m.swing();
                mark_swing(m.id);
                true
            }
            Species::Chicken | Species::Mooshroom => {
                let (item, text) = if species == Species::Chicken {
                    ("minecraft:feather", "Feather!")
                } else {
                    ("minecraft:red_mushroom", "Mushroom!")
                };
                drop(cx, item, 1);
                say(m.id, 2, text);
                m.swing();
                mark_swing(m.id);
                cx.events.push(MobEvent::AttackMob {
                    target,
                    amount: 0.0,
                    source: Damage::mob(m.id, here(m)),
                });
                true
            }
            Species::IronGolem => {
                if !self.rose_offered {
                    self.rose_offered = true;
                    cx.events.push(MobEvent::EntityEvent { mob: target, event: 11, at });
                }
                if m.random.next_float() >= 0.1 {
                    return false;
                }
                cx.events.push(MobEvent::EntityEvent { mob: target, event: 34, at });
                drop(cx, "minecraft:poppy", 1);
                say(m.id, 2, "Rose!");
                true
            }
            _ => true,
        }
    }
}

impl Goal for DollSteal {
    fn flags(&self) -> u8 {
        goals::MOVE | goals::LOOK
    }
    fn can_use(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        if !in_mode(m, PATROL) || cx.w.game_time - self.last_scan < 20 {
            return false;
        }
        self.last_scan = cx.w.game_time;
        self.target = nearest(m, cx, |s| s.species == Species::Sheep && shearable(s.id));
        // Each of the others gets a one in twenty chance a second.
        if self.target.is_none() && m.random.next_float() < 0.05 {
            self.target =
                nearest(m, cx, |s| s.species == Species::Chicken && !s.baby && !is_doll(s.id));
        }
        if self.target.is_none() && m.random.next_float() < 0.05 {
            self.target = nearest(m, cx, |s| s.species == Species::Mooshroom && !s.baby);
        }
        if self.target.is_none() && m.random.next_float() < 0.05 {
            self.target = nearest(m, cx, |s| s.species == Species::IronGolem);
        }
        self.target.is_some()
    }
    fn can_continue(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        in_mode(m, PATROL)
            && self.lost < LOST_AFTER
            && self.target.and_then(|id| cx.w.seen(id)).is_some_and(|s| s.alive)
    }
    fn start(&mut self, _: &mut Mob, _: &mut Cx) {
        self.lost = 0;
        self.rose_offered = false;
    }
    fn stop(&mut self, m: &mut Mob, _: &mut Cx) {
        self.target = None;
        m.nav_stop();
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        let Some(id) = self.target else { return };
        let Some((at, eye, species, color)) =
            cx.w.seen(id).map(|s| (pos(s), s.eye_y, s.species, s.color))
        else {
            return;
        };
        let sees = m.sees(cx.w, Who::Mob(id));
        if sees {
            m.look_at([at[0], eye, at[2]]);
            self.lost = (self.lost - 1).max(0);
        } else {
            self.lost += 1;
        }
        if cx.w.game_time - self.last_try < 20 {
            return;
        }
        self.last_try = cx.w.game_time;
        m.nav_move_to_entity(cx.w, at, 0, 1.0);
        if sees && in_reach(m, at) && self.take(m, cx, id, species, color, at) {
            self.target = None;
        }
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
    fn name(&self) -> &'static str {
        "DollSteal"
    }
}

/// Walks to the player, and teleports beside them from far away (`EntityDollAIFollowOwner`).
#[derive(Clone, Debug)]
struct DollFollow {
    recalculate_in: i32,
}

impl Goal for DollFollow {
    fn flags(&self) -> u8 {
        goals::MOVE | goals::LOOK
    }
    fn can_use(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        in_mode(m, FOLLOW)
            && bound_to(m.id).is_none()
            && owner_position(m, cx).is_some_and(|at| distance_sqr(m, at) >= FOLLOW_START * FOLLOW_START)
    }
    fn can_continue(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        in_mode(m, FOLLOW)
            && owner_position(m, cx).is_some_and(|at| distance_sqr(m, at) > FOLLOW_STOP * FOLLOW_STOP)
    }
    fn start(&mut self, _: &mut Mob, _: &mut Cx) {
        self.recalculate_in = 0;
    }
    fn stop(&mut self, m: &mut Mob, _: &mut Cx) {
        m.nav_stop();
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        let Some(at) = owner_position(m, cx) else { return };
        let eye = owner_seen(m, cx.w).map_or(at[1], |p| p.eye[1]);
        m.look_at([at[0], eye, at[2]]);
        self.recalculate_in -= 1;
        if self.recalculate_in > 0 {
            return;
        }
        self.recalculate_in = 10;
        if distance_sqr(m, at) >= TELEPORT_FROM * TELEPORT_FROM {
            teleport_near(m, cx, at);
        } else {
            m.nav_move_to(cx.w, at, 1.0);
        }
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
    fn name(&self) -> &'static str {
        "DollFollow"
    }
}

/// `TamableAnimal.tryToTeleportToOwner`: ten tries at a free block near the player.
fn teleport_near(m: &mut Mob, cx: &mut Cx, at: [f64; 3]) {
    let base = at.map(|c| c.floor() as i32);
    for _ in 0..10 {
        let dx = m.random.next_int_bound(7).unwrap_or(0) - 3;
        let dz = m.random.next_int_bound(7).unwrap_or(0) - 3;
        if dx.abs() < 2 && dz.abs() < 2 {
            continue;
        }
        let dy = m.random.next_int_bound(3).unwrap_or(0) - 1;
        let (x, y, z) = (base[0] + dx, base[1] + dy, base[2] + dz);
        let world = cx.w;
        let below = world.facts.get(world.world.block(x, y - 1, z));
        let here = world.facts.get(world.world.block(x, y, z));
        let head = world.facts.get(world.world.block(x, y + 1, z));
        if below.full_collision && !here.full_collision && !head.full_collision {
            m.position.x = f64::from(x) + 0.5;
            m.position.y = f64::from(y);
            m.position.z = f64::from(z) + 0.5;
            m.previous = m.position;
            m.nav_stop();
            // `teleportToXYZ`: mob.endermen.portal.
            let to = [m.position.x, m.position.y, m.position.z];
            cx.events.push(MobEvent::Sound("entity.enderman.teleport", to, 1.0, 1.0));
            return;
        }
    }
}

fn name_at(w: &MobWorld, p: [i32; 3]) -> Arc<str> {
    w.facts.get(w.world.block(p[0], p[1], p[2])).name.clone()
}

fn hardness_at(w: &MobWorld, p: [i32; 3]) -> f32 {
    w.facts.get(w.world.block(p[0], p[1], p[2])).hardness
}

/// `canDigBlock`: solid, no harder than obsidian, nothing that falls on top, and no drop into the
/// void or lava underneath.
fn can_dig(w: &MobWorld, p: [i32; 3]) -> bool {
    if is_placed(p) {
        return false;
    }
    let name = name_at(w, p);
    if is_doll_block(&name) {
        return false;
    }
    if is_air(&name) || matches!(&*name, "minecraft:water" | "minecraft:lava") {
        return false;
    }
    let hardness = hardness_at(w, p);
    if !(0.0..=HARDEST).contains(&hardness) {
        return false;
    }
    let above = name_at(w, [p[0], p[1] + 1, p[2]]);
    if matches!(&*above, "minecraft:sand" | "minecraft:red_sand" | "minecraft:gravel") {
        return false;
    }
    let mut y = p[1] - 1;
    while is_air(&name_at(w, [p[0], y, p[2]])) {
        if y <= WORLD_BOTTOM || &*name_at(w, [p[0], y - 1, p[2]]) == "minecraft:lava" {
            return false;
        }
        y -= 1;
    }
    true
}

/// The block at the doll's eyes, where the original measured from.
fn eye_block(m: &Mob) -> [i32; 3] {
    [
        m.position.x.floor() as i32,
        (m.position.y + m.kind.eye_height).floor() as i32,
        m.position.z.floor() as i32,
    ]
}

/// `EntityDollAIReportBlock`: every second, the ore within reach.
#[derive(Clone, Debug, Default)]
struct ReportOre {
    last: i64,
}

impl Goal for ReportOre {
    fn flags(&self) -> u8 {
        0
    }
    fn can_use(&mut self, m: &mut Mob, _: &mut Cx) -> bool {
        state(m.id).is_some_and(|s| s.enabled)
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        if cx.w.game_time - self.last < 20 {
            return;
        }
        self.last = cx.w.game_time;
        let eye = eye_block(m);
        let mut counts: BTreeMap<String, u32> = BTreeMap::new();
        let tell_treasure = secret(m);
        for d in around() {
            let name = name_at(cx.w, add(eye, d));
            if is_ore(&name) && (tell_treasure || !is_treasure(&name)) {
                *counts.entry(short(&name).to_owned()).or_default() += 1;
            }
        }
        if counts.is_empty() {
            say(m.id, 2, "No target");
        } else {
            let list: String = counts.iter().map(|(name, n)| format!("{name}[{n}]")).collect();
            say(m.id, 1, list);
        }
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
    fn name(&self) -> &'static str {
        "ReportOre"
    }
}

/// `EntityDollAIMineBlock`: on patrol, following, or riding its owner's head, digs towards the nearest ore it can reach,
/// one block at a time, taking as long over each as its hardness says.
#[derive(Clone, Debug, Default)]
struct MineOre {
    mine: [i32; 3],
    counter: i32,
    total: i32,
    last_scan: i64,
}

impl MineOre {
    /// The nearest ore, then the first block to dig on the way to it.
    fn find(&mut self, m: &Mob, w: &MobWorld) -> Option<[i32; 3]> {
        let eye = eye_block(m);
        let target = around()
            .filter(|&d| is_ore(&name_at(w, add(eye, d))) && can_dig(w, add(eye, d)))
            .min_by_key(|&d| steps(d, [0; 3]))?;
        // Step from the doll towards the ore, through air, until a block to dig.
        let mut at = [0; 3];
        for _ in 0..MINE_RANGE {
            let mut next = None;
            for d in [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]] {
                let cell = add(at, d);
                if steps(target, cell) >= steps(target, at) {
                    continue;
                }
                if can_dig(w, add(eye, cell)) {
                    next = Some((cell, true));
                } else if next.is_none() && is_air(&name_at(w, add(eye, cell))) {
                    next = Some((cell, false));
                }
            }
            let (cell, dig) = next?;
            at = cell;
            if dig {
                return Some(add(eye, at));
            }
        }
        None
    }
}

impl Goal for MineOre {
    fn flags(&self) -> u8 {
        goals::MOVE | goals::LOOK
    }
    fn can_use(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        if !(in_mode(m, PATROL) || in_mode(m, FOLLOW) || in_mode(m, RIDE))
            || cx.w.game_time - self.last_scan < 20
        {
            return false;
        }
        self.last_scan = cx.w.game_time;
        // On patrol the Netherite Horai quarries instead.
        if horai_tier(m) == 2 && in_mode(m, PATROL) {
            return false;
        }
        match self.find(m, cx.w) {
            Some(mine) => {
                self.mine = mine;
                true
            }
            None => false,
        }
    }
    fn can_continue(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        (in_mode(m, PATROL) || in_mode(m, FOLLOW) || in_mode(m, RIDE))
            && self.counter >= 0
            && !is_air(&name_at(cx.w, self.mine))
    }
    fn start(&mut self, m: &mut Mob, cx: &mut Cx) {
        let hardness = hardness_at(cx.w, self.mine);
        self.counter = quicker(dig_ticks(horai_tier(m), hardness), boost(m.id));
        self.total = self.counter.max(1);
        // On its owner's head it digs from where it sits, within reach of the head.
        if !in_mode(m, RIDE) {
            let at = self.mine.map(|c| f64::from(c) + 0.5);
            m.nav_move_to(cx.w, at, 1.0);
        }
    }
    fn stop(&mut self, m: &mut Mob, cx: &mut Cx) {
        m.nav_stop();
        cx.events.push(MobEvent::DestroyProgress { mob: m.id, pos: self.mine, progress: -1 });
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        let at = self.mine.map(|c| f64::from(c) + 0.5);
        m.look_at(at);
        if self.counter == 0 {
            break_out(m, cx, self.mine, at, true);
        } else {
            let progress = ((self.total - self.counter) * 10 / self.total).min(9);
            cx.events.push(MobEvent::DestroyProgress { mob: m.id, pos: self.mine, progress });
            if self.counter % 4 == 0 {
                let [x, y, z] = self.mine;
                let facts = cx.w.facts.get(cx.w.world.block(x, y, z));
                let sound = facts.sounds.break_sound.to_string();
                let volume = (facts.sounds.volume + 1.0) / 8.0;
                let pitch = facts.sounds.pitch * 0.5;
                cx.events.push(MobEvent::SoundId(sound, at, volume, pitch));
            }
        }
        if self.counter % 6 == 0 {
            m.swing();
            mark_swing(m.id);
        }
        self.counter -= 1;
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
    fn name(&self) -> &'static str {
        "MineOre"
    }
}

/// On patrol, walks up to the nearest hostile mob, fuses for a second and a half, and explodes.
/// Unlike the original's, it lives through it: no blast hurts it (the api crate's
/// `living_incoming_damage`), and it rests for five seconds before the next.
#[derive(Clone, Debug, Default)]
struct Explode {
    target: Option<u32>,
    fuse: i32,
    last_scan: i64,
    last_path: i64,
    rest_until: i64,
}

impl Goal for Explode {
    fn flags(&self) -> u8 {
        goals::MOVE | goals::LOOK
    }
    fn can_use(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        if !in_mode(m, PATROL) || cx.w.game_time - self.last_scan < 20 || cx.w.game_time < self.rest_until {
            return false;
        }
        self.last_scan = cx.w.game_time;
        self.target = nearest(m, cx, |s| patrol_target(s) && !is_doll(s.id));
        match self.target.and_then(|id| cx.w.seen(id)) {
            Some(s) => say(m.id, 1, format!("{:?}({})", s.species, s.id)),
            None => say(m.id, 2, "No target"),
        }
        self.target.is_some()
    }
    fn can_continue(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        in_mode(m, PATROL) && self.target.and_then(|id| cx.w.seen(id)).is_some_and(|s| s.alive)
    }
    fn start(&mut self, _: &mut Mob, _: &mut Cx) {
        self.fuse = 0;
        self.last_path = 0;
    }
    fn stop(&mut self, m: &mut Mob, _: &mut Cx) {
        self.target = None;
        m.nav_stop();
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        let Some(id) = self.target else { return };
        let Some((at, eye)) = cx.w.seen(id).map(|s| (pos(s), s.eye_y)) else { return };
        m.look_at([at[0], eye, at[2]]);
        if cx.w.game_time - self.last_path >= 20 {
            self.last_path = cx.w.game_time;
            m.nav_move_to_entity(cx.w, at, 0, 1.0);
        }
        if distance_sqr(m, at) >= 5.0 {
            self.fuse = (self.fuse - 1).max(0);
            return;
        }
        self.fuse += 1;
        let me = here(m);
        if self.fuse == 1 {
            cx.events.push(MobEvent::Sound("entity.creeper.primed", me, 1.0, 0.5));
        }
        if self.fuse >= FUSE {
            cx.events.push(MobEvent::Explode(me, blast(m.id, EXPLODE_STRENGTH), m.species(), false));
            blasted(m.id);
            self.fuse = 0;
            self.target = None;
            self.rest_until = cx.w.game_time + BLAST_REST;
        }
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
    fn name(&self) -> &'static str {
        "Explode"
    }
}

/// On patrol or following, finds a tree (a trunk with leaves around it), hops to its top, and
/// chops it down layer by layer.
#[derive(Clone, Debug, Default)]
struct CutTree {
    /// The trunk's top block still standing, and what it is made of.
    target: [i32; 3],
    log: Option<Arc<str>>,
    cut: [i32; 3],
    counter: i32,
    strength: i32,
    last_scan: i64,
    /// The block being cut is the trunk's bottom one: a sapling goes in where it stood.
    last: bool,
}

/// The sapling a log grows from (a mangrove's propagule, a crimson or warped stem's fungus): oak
/// log, oak sapling, and so on for any wood named that way.
fn sapling_for(log: &str) -> Option<&'static str> {
    static NAMES: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<String, &'static str>>> =
        std::sync::LazyLock::new(Default::default);
    let name = match log {
        "minecraft:mangrove_log" => "minecraft:mangrove_propagule".to_owned(),
        "minecraft:crimson_stem" => "minecraft:crimson_fungus".to_owned(),
        "minecraft:warped_stem" => "minecraft:warped_fungus".to_owned(),
        _ => format!("{}_sapling", log.strip_suffix("_log")?),
    };
    let mut names = NAMES.lock().ok()?;
    if let Some(n) = names.get(&name) {
        return Some(n);
    }
    let leaked: &'static str = Box::leak(name.clone().into_boxed_str());
    names.insert(name, leaked);
    Some(leaked)
}

impl CutTree {
    /// The nearest tree: a trunk within `CUT_RANGE` across and from `TRUNK_BELOW` below its eyes to
    /// `TRUNK_ABOVE` above them (a tree on a slope, or one it stands below), crowned with leaves (at
    /// least three beside it, or leaves on top). Its top block, the leaves round it and how tall it is.
    fn find(&self, m: &Mob, w: &MobWorld) -> Option<([i32; 3], Arc<str>, u32, u32)> {
        let [x, y, z] = eye_block(m);
        let mut best: Option<([i32; 3], Arc<str>, u32, u32)> = None;
        let mut best_steps = i32::MAX;
        for dx in -CUT_RANGE..=CUT_RANGE {
            for dz in -CUT_RANGE..=CUT_RANGE {
                let steps = dx.abs() + dz.abs();
                if steps > CUT_RANGE || steps >= best_steps {
                    continue;
                }
                let (cx, cz) = (x + dx, z + dz);
                let Some(found) = (y - TRUNK_BELOW..=y + TRUNK_ABOVE).find(|&ty| is_log(&name_at(w, [cx, ty, cz]))) else {
                    continue;
                };
                let log = name_at(w, [cx, found, cz]);
                let mut ty = found;
                while ty > WORLD_BOTTOM && name_at(w, [cx, ty - 1, cz]) == log {
                    ty -= 1;
                }
                let (mut height, mut leaves) = (0, 0);
                while name_at(w, [cx, ty, cz]) == log && height < 64 {
                    for (ox, oz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                        if is_leafy(&name_at(w, [cx + ox, ty, cz + oz])) {
                            leaves += 1;
                        }
                    }
                    height += 1;
                    ty += 1;
                }
                let crowned = is_leafy(&name_at(w, [cx, ty, cz]));
                if leaves >= 3 || (crowned && leaves >= 1) {
                    best = Some(([cx, ty - 1, cz], log, leaves, height));
                    best_steps = steps;
                }
            }
        }
        best
    }
}

impl Goal for CutTree {
    fn flags(&self) -> u8 {
        goals::MOVE | goals::LOOK
    }
    fn can_use(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        if !(in_mode(m, PATROL) || in_mode(m, FOLLOW) || in_mode(m, RIDE))
            || cx.w.game_time - self.last_scan < 20
        {
            return false;
        }
        self.last_scan = cx.w.game_time;
        let Some((top, log, leaves, height)) = self.find(m, cx.w) else {
            say(m.id, 2, "No target");
            return false;
        };
        say(m.id, 1, format!("Target[{leaves},{height}]"));
        self.target = top;
        self.log = Some(log);
        // On its owner's head it chops from where it sits; otherwise `teleportToXYZ` puts it
        // on top of the trunk.
        if !in_mode(m, RIDE) {
            m.position.x = f64::from(top[0]) + 0.5;
            m.position.y = f64::from(top[1] + 1);
            m.position.z = f64::from(top[2]) + 0.5;
            m.previous = m.position;
            m.nav_stop();
            cx.events.push(MobEvent::Sound("entity.enderman.teleport", here(m), 1.0, 1.0));
        }
        true
    }
    fn can_continue(&mut self, m: &mut Mob, _: &mut Cx) -> bool {
        let near = (m.position.x.floor() as i32 - self.target[0]).abs()
            + (m.position.y.floor() as i32 - self.target[1]).abs()
            + (m.position.z.floor() as i32 - self.target[2]).abs()
            <= CUT_RANGE;
        (in_mode(m, PATROL) || in_mode(m, FOLLOW) || in_mode(m, RIDE)) && self.log.is_some() && near
    }
    fn start(&mut self, _: &mut Mob, _: &mut Cx) {
        self.counter = 0;
    }
    fn stop(&mut self, m: &mut Mob, cx: &mut Cx) {
        self.log = None;
        cx.events.push(MobEvent::DestroyProgress { mob: m.id, pos: self.cut, progress: -1 });
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        let Some(log) = self.log.clone() else { return };
        let w = cx.w;
        if self.counter == 0 {
            // Another log of this layer within reach first (branches, a wide trunk), else the trunk,
            // a layer lower each time.
            let [tx, ty, tz] = self.target;
            let mut branch = None;
            'search: for dx in -CUT_RANGE..=CUT_RANGE {
                for dz in -CUT_RANGE..=CUT_RANGE {
                    if dx.abs() + dz.abs() <= CUT_RANGE && name_at(w, [tx + dx, ty, tz + dz]) == log {
                        branch = Some([tx + dx, ty, tz + dz]);
                        break 'search;
                    }
                }
            }
            match branch {
                Some(at) => self.cut = at,
                None => {
                    if name_at(w, self.target) != log {
                        self.log = None;
                        return;
                    }
                    self.cut = self.target;
                    self.target[1] -= 1;
                    // The trunk's bottom block: cut too, and a sapling put in its place.
                    self.last = name_at(w, self.target) != log;
                }
            }
            let hardness = hardness_at(w, self.cut);
            // Half as quick again as a plain axe's pace.
            self.strength = ((1.0 + 30.0 * hardness / CUT_SPEED) / 1.5).floor().max(0.0) as i32;
        }
        let at = self.cut.map(|c| f64::from(c) + 0.5);
        m.look_at(at);
        if self.counter >= self.strength {
            cx.events.push(MobEvent::BreakBlock { pos: self.cut, drop: true, by: None, breaker: Some(m.id) });
            worked(m.id);
            self.counter = 0;
            if std::mem::take(&mut self.last) {
                // A sapling of the same wood, if she carries one, where the tree stood.
                let ground = name_at(w, add(self.cut, [0, -1, 0]));
                if let Some(sapling) = sapling_for(&log)
                    && !is_air(&ground)
                    && !is_log(&ground)
                    && take_one(m.id, sapling)
                {
                    cx.events.push(MobEvent::SetBlockName { mob: Some(m.id), pos: self.cut, name: sapling, effect: false, flags: 3 });
                    cx.events.push(MobEvent::Sound("block.grass.place", at, 1.0, 0.8));
                }
                self.log = None;
            }
            return;
        }
        let progress = (self.counter * 10 / self.strength.max(1)).min(9);
        cx.events.push(MobEvent::DestroyProgress { mob: m.id, pos: self.cut, progress });
        if self.counter % 4 == 0 {
            let [x, y, z] = self.cut;
            let facts = w.facts.get(w.world.block(x, y, z));
            let sound = facts.sounds.break_sound.to_string();
            let volume = (facts.sounds.volume + 1.0) / 8.0;
            let pitch = facts.sounds.pitch * 0.5;
            cx.events.push(MobEvent::SoundId(sound, at, volume, pitch));
        }
        if self.counter % 6 == 0 {
            m.swing();
            mark_swing(m.id);
        }
        self.counter += 1;
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
    fn name(&self) -> &'static str {
        "CutTree"
    }
}

// ---- London: EntityDollAIReportArtifact ----

/// In any mode, sweeps the layers above and below it, 64 each way, over the chunks around it, one
/// layer a tick, and reports where the spawners are: `-` every 8 layers, a `[spawner]` at each
/// layer that has one, and `<->` at its own height (the original showed the stronghold's direction
/// there; Horizon gives a mob no way to find it).
#[derive(Clone, Debug, Default)]
struct ReportSpawners {
    counter: i32,
    chunk: [i32; 2],
    base_y: i32,
    found: BTreeMap<i32, Vec<String>>,
}

impl Goal for ReportSpawners {
    fn flags(&self) -> u8 {
        0
    }
    fn can_use(&mut self, m: &mut Mob, _: &mut Cx) -> bool {
        state(m.id).is_some_and(|s| s.enabled)
    }
    fn start(&mut self, _: &mut Mob, _: &mut Cx) {
        self.counter = 0;
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        if self.counter == 0 {
            self.chunk = [(m.position.x.floor() as i32).div_euclid(16), (m.position.z.floor() as i32).div_euclid(16)];
            self.base_y = (m.position.y + m.kind.eye_height).floor() as i32;
            self.found.clear();
        }
        let [cx0, cz0] = self.chunk;
        for dy in [-1, 1] {
            if self.counter == 0 && dy == 1 {
                continue;
            }
            let layer = self.counter * dy;
            let y = self.base_y + layer;
            for dx in -16..=32 {
                for dz in -16..=32 {
                    let name = name_at(cx.w, [cx0 * 16 + dx, y, cz0 * 16 + dz]);
                    if matches!(&*name, "minecraft:spawner" | "minecraft:trial_spawner") {
                        self.found.entry(layer).or_default().push(short(&name).to_owned());
                    }
                }
            }
        }
        if self.counter == 63 {
            let mut text = String::new();
            let mark = |text: &mut String, layer: i32, found: &BTreeMap<i32, Vec<String>>| {
                if let Some(names) = found.get(&layer) {
                    text.push_str(&format!("[{}]", names.join(",")));
                }
            };
            for i in (1..=64).rev() {
                if i % 8 == 0 {
                    text.push('-');
                }
                mark(&mut text, i, &self.found);
            }
            match self.found.get(&0) {
                Some(names) => text.push_str(&format!("<[{}]>", names.join(","))),
                None => text.push_str("<->"),
            }
            for i in (-64..=-1).rev() {
                mark(&mut text, i, &self.found);
                if i % 8 == 0 {
                    text.push('-');
                }
            }
            say(m.id, 2, text);
        }
        self.counter = (self.counter + 1) % 64;
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
    fn name(&self) -> &'static str {
        "ReportSpawners"
    }
}

// ---- Russia: EntityDollAICollectItem and EntityDollAITorcher ----

/// On patrol or following, walks to the dropped item the api crate found for it; the api crate
/// takes the item into its inventory once it is close.
#[derive(Clone, Debug, Default)]
struct CollectItem {
    last_path: i64,
}

impl Goal for CollectItem {
    fn flags(&self) -> u8 {
        goals::MOVE | goals::LOOK
    }
    fn can_use(&mut self, m: &mut Mob, _: &mut Cx) -> bool {
        item_target(m.id).is_some()
    }
    fn start(&mut self, _: &mut Mob, _: &mut Cx) {
        self.last_path = 0;
    }
    fn stop(&mut self, m: &mut Mob, _: &mut Cx) {
        m.nav_stop();
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        let Some(at) = item_target(m.id) else { return };
        m.look_at(at);
        if cx.w.game_time - self.last_path >= 10 {
            self.last_path = cx.w.game_time;
            m.nav_move_to(cx.w, at, 1.0);
        }
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
    fn name(&self) -> &'static str {
        "CollectItem"
    }
}

/// With torches in its first slot, puts one down wherever it walks that is dark enough for
/// monsters to spawn, in any mode but Standby.
#[derive(Clone, Debug, Default)]
struct Torcher {
    last: i64,
}

impl Goal for Torcher {
    fn flags(&self) -> u8 {
        0
    }
    fn can_use(&mut self, m: &mut Mob, _: &mut Cx) -> bool {
        state(m.id).is_some_and(|s| s.enabled && s.mode != STANDBY)
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        if cx.w.game_time - self.last < 20 {
            return;
        }
        self.last = cx.w.game_time;
        let first_is_torch = inventory(m.id)
            .and_then(|items| items.first().cloned().flatten())
            .is_some_and(|s| &*s.item == "minecraft:torch" && s.count > 0);
        if !first_is_torch {
            return;
        }
        let w = cx.w;
        let [x, eye, z] = eye_block(m);
        // The first air above the ground under its eyes.
        let mut y = eye;
        while !is_air(&name_at(w, [x, y, z])) && y < eye + 4 {
            y += 1;
        }
        while is_air(&name_at(w, [x, y, z])) && y > eye - 5 {
            y -= 1;
        }
        y += 1;
        if y < eye - 3 || y > eye + 3 || !is_air(&name_at(w, [x, y, z])) {
            return;
        }
        let light = i32::from(w.world.light(x, y, z));
        let (sky, block) = (light >> 4, light & 15);
        if sky > m.random.next_int_bound(32).unwrap_or(0) {
            return;
        }
        let darken = if w.thundering { 10 } else if w.daylight { 0 } else { 11 };
        if block.max(sky - darken) > LIGHT_THRESHOLD {
            return;
        }
        if !w.facts.get(w.world.block(x, y - 1, z)).full_collision {
            return;
        }
        let took = edit_inventory(m.id, |items| match items.first_mut() {
            Some(Some(s)) if &*s.item == "minecraft:torch" && s.count > 0 => {
                s.count -= 1;
                if s.count == 0 {
                    items[0] = None;
                }
                true
            }
            _ => false,
        });
        if took == Some(true) {
            cx.events.push(MobEvent::SetBlockName {
                mob: Some(m.id),
                pos: [x, y, z],
                name: "minecraft:torch",
                effect: false,
                flags: 3,
            });
            worked(m.id);
            m.swing();
            mark_swing(m.id);
        }
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
    fn name(&self) -> &'static str {
        "Torcher"
    }
}

/// Marisa (on patrol, following, or on her owner's head) keeps within ten blocks of a hostile mob
/// and holds a beam on it. Reimu (in every mode) stands her ground and fires a ring of eight amulets
/// every four seconds, walking up only when she cannot see it. Neither walks while riding.
#[derive(Clone, Debug)]
struct Danmaku {
    shot: Shot,
    target: Option<u32>,
    bullets: Vec<Bullet>,
    last_scan: i64,
    counter: i32,
    lost: i32,
    /// Ticks to the beam's next hit.
    hit: i32,
}

impl Danmaku {
    fn new(shot: Shot) -> Self {
        Self { shot, target: None, bullets: Vec::new(), last_scan: 0, counter: 0, lost: 0, hit: 0 }
    }
    fn allowed(&self, m: &Mob) -> bool {
        match self.shot {
            Shot::Beam => in_mode(m, PATROL) || in_mode(m, FOLLOW) || in_mode(m, RIDE),
            Shot::Amulet => state(m.id).is_some_and(|s| s.enabled && s.mode != SECRET && s.mode != SECRET_ALL),
        }
    }
    /// Holds the beam on the target, and hurts it every `BEAM_EVERY` ticks.
    fn beam(&mut self, m: &mut Mob, cx: &mut Cx, target: u32, aim: [f64; 3]) {
        let muzzle = [m.position.x, m.position.y + m.kind.eye_height, m.position.z];
        set_beam(m.id, Some((muzzle, aim)));
        let t = f64::from((cx.w.game_time % 8) as i32) / 8.0;
        let along = [0, 1, 2].map(|i| muzzle[i] + (aim[i] - muzzle[i]) * t);
        cx.events.push(MobEvent::ParticleAt { kind: "end_rod", at: along, velocity: [0.0; 3] });
        self.hit -= 1;
        if self.hit > 0 {
            return;
        }
        self.hit = BEAM_EVERY;
        if !is_doll(target) {
            // DamageSource.causePlayerDamage(owner): the kill is the owner's.
            cx.events.push(MobEvent::AttackMob { target, amount: attack(m.id, BEAM_DAMAGE), source: Damage::player(here(m)) });
            worked(m.id);
        }
        cx.events.push(MobEvent::ParticleAt { kind: "flash", at: aim, velocity: [0.0; 3] });
        cx.events.push(MobEvent::Sound("entity.blaze.shoot", muzzle, 0.25, 2.0));
        m.swing();
        mark_swing(m.id);
    }
    fn wanted(m: &Mob, cx: &Cx, s: &Seen) -> bool {
        // A mob that targets it or hurt it or its owner, it shoots back at, wherever it is.
        if fights_back(m, s) {
            return true;
        }
        if !enemy(s) || spares(m.id, Who::Mob(s.id)) {
            return false;
        }
        // Following or riding, it guards its owner and does not go after anything far from them.
        !(in_mode(m, FOLLOW) || in_mode(m, RIDE)) || matches!(s.target, Some(Who::Player)) || near_owner(m, cx, s)
    }
    fn fire(&mut self, m: &mut Mob, cx: &mut Cx, target: u32, at: [f64; 3]) {
        let muzzle = [m.position.x, m.position.y + m.kind.eye_height, m.position.z];
        let (dx, dz) = (at[0] - muzzle[0], at[2] - muzzle[2]);
        let across = (dx * dx + dz * dz).sqrt().max(0.01);
        let side = [-dz / across, 0.0, dx / across];
        match self.shot {
            Shot::Beam => {}
            Shot::Amulet => {
                for i in 0..8 {
                    let a = f64::from(i) * std::f64::consts::FRAC_PI_4;
                    let (c, s) = (a.cos() * 0.8, a.sin() * 0.8);
                    self.bullets.push(Bullet {
                        at: [muzzle[0] + side[0] * c, muzzle[1] + s, muzzle[2] + side[2] * c],
                        target,
                        age: -(i % 4),
                    });
                }
                cx.events.push(MobEvent::Sound("entity.arrow.shoot", muzzle, 1.0, 0.8));
            }
        }
        m.swing();
        mark_swing(m.id);
    }
    fn fly(&mut self, m: &Mob, cx: &mut Cx) {
        let (speed, damage, color, sparkle) = self.shot.look();
        let damage = attack(m.id, damage);
        let mut drawn = Vec::new();
        let from = here(m);
        let id = m.id;
        let w = cx.w;
        let events = &mut cx.events;
        self.bullets.retain_mut(|b| {
            b.age += 1;
            if b.age <= 0 {
                return true;
            }
            if b.age > BULLET_LIFE {
                return false;
            }
            let Some(s) = w.seen(b.target).filter(|s| s.alive) else { return false };
            let aim = [s.position.x, (s.position.y + s.eye_y) / 2.0, s.position.z];
            let d = [aim[0] - b.at[0], aim[1] - b.at[1], aim[2] - b.at[2]];
            let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            if len <= speed + 0.4 {
                // It strikes only the mob it was fired at, and never a doll: a shot can't hurt its
                // owner (a player is never a doll's target) or any of their dolls.
                if !is_doll(b.target) {
                    // DamageSource.causePlayerDamage(owner): the kill is the owner's.
                    events.push(MobEvent::AttackMob { target: b.target, amount: damage, source: Damage::player(from) });
                    worked(id);
                }
                events.push(MobEvent::ParticleAt { kind: sparkle, at: aim, velocity: [0.0; 3] });
                return false;
            }
            let was = b.at;
            for c in 0..3 {
                b.at[c] += d[c] / len * speed;
            }
            drawn.push((b.at, was));
            events.push(MobEvent::ParticleColored { at: b.at, color });
            events.push(MobEvent::ParticleAt { kind: sparkle, at: b.at, velocity: [0.0; 3] });
            true
        });
        if let Ok(mut f) = FLYING.lock() {
            let kind = match self.shot {
                Shot::Beam => 0,
                Shot::Amulet => 1,
            };
            if drawn.is_empty() {
                f.remove(&m.id);
            } else {
                f.insert(m.id, (kind, drawn));
            }
        }
    }
}

impl Goal for Danmaku {
    fn flags(&self) -> u8 {
        goals::MOVE | goals::LOOK
    }
    fn can_use(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        if !self.allowed(m) || cx.w.game_time - self.last_scan < 20 {
            return false;
        }
        self.last_scan = cx.w.game_time;
        let from = here(m);
        // Whatever is attacking it comes first.
        self.target = my_attacker(m, cx).or_else(|| {
            cx.w.others
                .iter()
                .filter(|s| s.id != m.id && noticed(from, s) && Self::wanted(m, cx, s))
                .min_by(|a, b| distance_sqr(m, pos(a)).total_cmp(&distance_sqr(m, pos(b))))
                .map(|s| s.id)
        });
        self.target.is_some()
    }
    fn can_continue(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        if self.allowed(m) && attacker_elsewhere(m, cx, self.target) {
            return false;
        }
        let hunting = self.allowed(m)
            && self.lost < LOST_AFTER
            && self.target.and_then(|id| cx.w.seen(id)).is_some_and(|s| s.alive);
        hunting || !self.bullets.is_empty()
    }
    fn start(&mut self, _: &mut Mob, _: &mut Cx) {
        self.counter = 0;
        self.lost = 0;
    }
    fn stop(&mut self, m: &mut Mob, _: &mut Cx) {
        self.target = None;
        self.bullets.clear();
        if let Ok(mut f) = FLYING.lock() {
            f.remove(&m.id);
        }
        set_beam(m.id, None);
        m.nav_stop();
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        self.fly(m, cx);
        let Some(id) = self.target else { return };
        let Some((at, eye, alive)) = cx.w.seen(id).map(|s| (pos(s), s.eye_y, s.alive)) else {
            self.target = None;
            set_beam(m.id, None);
            return;
        };
        if !alive || !self.allowed(m) {
            self.target = None;
            set_beam(m.id, None);
            m.nav_stop();
            return;
        }
        let sees = m.sees(cx.w, Who::Mob(id));
        if sees {
            m.look_at([at[0], eye, at[2]]);
            self.lost = 0;
        } else {
            self.lost += 1;
        }
        let aim = [at[0], (at[1] + eye) / 2.0, at[2]];
        self.counter -= 1;
        match self.shot {
            Shot::Beam => {
                if self.counter <= 0 {
                    self.counter = 10;
                    if in_mode(m, RIDE) {
                    } else if distance_sqr(m, at) > 100.0 || !sees {
                        m.nav_move_to_entity(cx.w, at, 0, 1.0);
                    } else {
                        m.nav_stop();
                    }
                }
                if sees && distance_sqr(m, at) <= BEAM_RANGE * BEAM_RANGE {
                    self.beam(m, cx, id, aim);
                } else {
                    set_beam(m.id, None);
                }
            }
            Shot::Amulet => {
                if self.counter % 20 == 0 {
                    if !sees && (in_mode(m, PATROL) || in_mode(m, FOLLOW)) {
                        m.nav_move_to_entity(cx.w, at, 0, 1.0);
                    } else {
                        m.nav_stop();
                    }
                }
                if self.counter <= 0 {
                    if sees {
                        self.counter = 80;
                        self.fire(m, cx, id, aim);
                    } else {
                        self.counter = 20;
                    }
                }
            }
        }
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
    fn name(&self) -> &'static str {
        "Danmaku"
    }
}

/// Who an orb is for: a mob, or a player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mark {
    Mob(u32),
    Player(u128),
}

/// Patchouli's orb in flight: it homes in on its mark and hits nothing else.
#[derive(Clone, Debug)]
struct Orb {
    at: [f64; 3],
    mark: Mark,
    damage: f32,
    age: i32,
}

/// Patchouli (on patrol, following, on her owner's head, or in her other mode) fires one slow,
/// heavy orb at a time at whatever is in range and in sight, whatever is attacking her first. She
/// doesn't walk up to it: she keeps on with what her mode has her do.
#[derive(Clone, Debug, Default)]
struct Snipe {
    mark: Option<Mark>,
    orb: Option<Orb>,
    counter: i32,
    lost: i32,
    last_scan: i64,
}

impl Snipe {
    fn allowed(m: &Mob) -> bool {
        in_mode(m, PATROL) || in_mode(m, FOLLOW) || in_mode(m, RIDE) || in_mode(m, SNIPE)
    }
    fn range(m: &Mob) -> f64 {
        if in_mode(m, SNIPE) { SNIPE_SECRET_RANGE } else { SNIPE_RANGE }
    }
    /// Where `mark` stands, and the middle of it to aim at, while it is alive and in her level.
    fn where_is(w: &MobWorld, mark: Mark) -> Option<([f64; 3], [f64; 3])> {
        match mark {
            Mark::Mob(id) => w
                .seen(id)
                .filter(|s| s.alive && !off_limits(s))
                .map(|s| (pos(s), [s.position.x, (s.position.y + s.eye_y) / 2.0, s.position.z])),
            Mark::Player(p) => {
                let o = OWNERS.lock().ok()?.get(&p).copied()?;
                (o.dimension == world_dimension(w) && !o.spectator)
                    .then_some((o.position, [o.position[0], (o.position[1] + o.eye[1]) / 2.0, o.position[2]]))
            }
        }
    }
    /// Whether she has a clear shot at `aim`.
    fn in_sight(m: &mut Mob, w: &MobWorld, mark: Mark, aim: [f64; 3]) -> bool {
        match mark {
            Mark::Mob(id) => m.sees(w, Who::Mob(id)),
            Mark::Player(_) => {
                let eye = horizon_core::geometry::Vec3::new(m.position.x, m.position.y + m.kind.eye_height, m.position.z);
                w.line_of_sight_for(Some(m.position.y), eye, horizon_core::geometry::Vec3::new(aim[0], aim[1], aim[2]))
            }
        }
    }
    /// What she shoots next: whatever is attacking her, else the nearest she has a clear shot at.
    fn pick(m: &mut Mob, cx: &Cx) -> Option<Mark> {
        let secret = in_mode(m, SNIPE);
        let range = Self::range(m);
        let from = here(m);
        if let Some(a) = my_attacker(m, cx)
            && cx.w.seen(a).is_some_and(|s| !off_limits(s) && snipe_reach(pos(s), from, range))
        {
            return Some(Mark::Mob(a));
        }
        let owner = state(m.id).map_or(0, |s| s.owner);
        let mut marks: Vec<(f64, Mark)> = cx
            .w
            .others
            .iter()
            .filter(|s| s.id != m.id && s.alive && !off_limits(s) && snipe_reach(pos(s), from, range))
            .filter(|s| {
                if secret {
                    // Chickens are beneath her notice.
                    s.species != Species::Chicken && !spares(m.id, Who::Mob(s.id))
                } else {
                    fights_back(m, s) || (enemy(s) && !spares(m.id, Who::Mob(s.id)))
                }
            })
            .map(|s| (distance_sqr(m, pos(s)), Mark::Mob(s.id)))
            .collect();
        let dim = world_dimension(cx.w);
        let foes = grudge(m);
        if let Ok(players) = OWNERS.lock() {
            for (&p, o) in players.iter() {
                let wanted = secret || foes.contains(&Foe::Player(p));
                if p != owner && wanted && o.dimension == dim && !o.spectator && snipe_reach(o.position, from, range) {
                    marks.push((distance_sqr(m, o.position), Mark::Player(p)));
                }
            }
        }
        marks.sort_by(|a, b| a.0.total_cmp(&b.0));
        marks.into_iter().take(SNIPE_LOOKS).map(|(_, k)| k).find(|&k| {
            Self::where_is(cx.w, k).is_some_and(|(_, aim)| Self::in_sight(m, cx.w, k, aim))
        })
    }
    fn fire(&mut self, m: &mut Mob, cx: &mut Cx, mark: Mark) {
        let muzzle = [m.position.x, m.position.y + m.kind.eye_height, m.position.z];
        let base = attack(m.id, SNIPE_DAMAGE);
        let damage = if in_mode(m, SNIPE) { base * 2.0 } else { base };
        self.orb = Some(Orb { at: muzzle, mark, damage, age: 0 });
        cx.events.push(MobEvent::Sound("entity.evoker.cast_spell", muzzle, 1.0, 0.8));
        m.swing();
        mark_swing(m.id);
    }
    fn fly(&mut self, m: &Mob, cx: &mut Cx) {
        let Some(orb) = self.orb.as_mut() else { return };
        orb.age += 1;
        let was = orb.at;
        let landed = match Self::where_is(cx.w, orb.mark) {
            None => true,
            Some(_) if orb.age > ORB_LIFE => true,
            Some((_, aim)) => {
                let d = [aim[0] - orb.at[0], aim[1] - orb.at[1], aim[2] - orb.at[2]];
                let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                if len <= ORB_SPEED + 0.5 {
                    match orb.mark {
                        // DamageSource.causePlayerDamage(owner): the kill is the owner's.
                        // Never a doll, whoever's, even one it was not meant for.
                        Mark::Mob(target) => {
                            let doll = cx.w.seen(target).is_some_and(|s| s.kind.id == DOLL_TYPE || is_doll(s.id));
                            let amount = if doll { 0.0 } else { orb_damage(target, orb.damage) };
                            if amount > 0.0 {
                                cx.events.push(MobEvent::AttackMob { target, amount, source: Damage::player(here(m)) });
                                worked(m.id);
                            }
                        }
                        Mark::Player(p) => hit_player(m.id, p, orb.damage),
                    }
                    cx.events.push(MobEvent::ParticleAt { kind: "flash", at: aim, velocity: [0.0; 3] });
                    cx.events.push(MobEvent::ParticleAt { kind: "witch", at: aim, velocity: [0.0; 3] });
                    cx.events.push(MobEvent::Sound("entity.firework_rocket.blast", aim, 1.0, 0.6));
                    true
                } else {
                    for c in 0..3 {
                        orb.at[c] += d[c] / len * ORB_SPEED;
                    }
                    cx.events.push(MobEvent::ParticleColored { at: orb.at, color: [0.7, 0.35, 1.0] });
                    cx.events.push(MobEvent::ParticleAt { kind: "witch", at: orb.at, velocity: [0.0; 3] });
                    false
                }
            }
        };
        let at = orb.at;
        if landed {
            self.orb = None;
        }
        if let Ok(mut f) = FLYING.lock() {
            if landed {
                f.remove(&m.id);
            } else {
                f.insert(m.id, (2, vec![(at, was)]));
            }
        }
    }
}

impl Goal for Snipe {
    fn flags(&self) -> u8 {
        goals::LOOK
    }
    fn can_use(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        if !Self::allowed(m) || cx.w.game_time - self.last_scan < 20 {
            return false;
        }
        self.last_scan = cx.w.game_time;
        self.mark = Self::pick(m, cx);
        self.mark.is_some()
    }
    fn can_continue(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        let mob = match self.mark {
            Some(Mark::Mob(id)) => Some(id),
            _ => None,
        };
        if Self::allowed(m) && self.orb.is_none() && attacker_elsewhere(m, cx, mob) {
            return false;
        }
        let range = Self::range(m) * 1.1;
        let from = here(m);
        let hunting = Self::allowed(m)
            && self.lost < LOST_AFTER
            && self.mark.and_then(|k| Self::where_is(cx.w, k)).is_some_and(|(at, _)| snipe_reach(at, from, range));
        hunting || self.orb.is_some()
    }
    fn start(&mut self, _: &mut Mob, _: &mut Cx) {
        self.counter = 10;
        self.lost = 0;
    }
    fn stop(&mut self, m: &mut Mob, _: &mut Cx) {
        self.mark = None;
        self.orb = None;
        if let Ok(mut f) = FLYING.lock() {
            f.remove(&m.id);
        }
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        self.fly(m, cx);
        let Some(mark) = self.mark else { return };
        let Some((_, aim)) = Self::where_is(cx.w, mark).filter(|_| Self::allowed(m)) else {
            self.mark = None;
            return;
        };
        m.look_at(aim);
        self.counter -= 1;
        if self.counter > 0 || self.orb.is_some() {
            return;
        }
        if Self::in_sight(m, cx.w, mark, aim) {
            self.lost = 0;
            self.counter = if in_mode(m, SNIPE) { SNIPE_EVERY } else { SNIPE_EVERY_PATROL };
            self.fire(m, cx, mark);
        } else {
            self.lost += 10;
            self.counter = 10;
        }
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
    fn name(&self) -> &'static str {
        "Snipe"
    }
}

/// Every thirty seconds, and straight away when she is hurt (`emergencyShot`), Aya takes a photo:
/// the shutter clicks, and she tells her owner what she caught. (The original saved a screenshot
/// on the owner's computer.)
#[derive(Clone, Debug, Default)]
struct AyaShot {
    counter: i32,
    health: f32,
    /// Ticks since her last photo kept as a screenshot.
    film: i32,
}

impl AyaShot {
    fn photo(&self, m: &mut Mob, cx: &mut Cx, hurt: bool) {
        let me = here(m);
        let head = [me[0], me[1] + m.kind.eye_height, me[2]];
        cx.events.push(MobEvent::Sound("block.wool.break", head, 0.2, 2.0));
        cx.events.push(MobEvent::Sound("ui.button.click", head, 0.2, 2.0));
        for (dx, dz) in [(0.3, 0.0), (-0.3, 0.0), (0.0, 0.3), (0.0, -0.3)] {
            let at = [head[0] + dx, head[1] + 0.2, head[2] + dz];
            cx.events.push(MobEvent::ParticleAt { kind: "end_rod", at, velocity: [dx * 0.1, 0.05, dz * 0.1] });
        }
        m.swing();
        mark_swing(m.id);
        let mut counts: BTreeMap<String, u32> = BTreeMap::new();
        for s in cx.w.others.iter().filter(|s| s.id != m.id && noticed(me, s) && !is_doll(s.id)) {
            *counts.entry(format!("{:?}", s.species)).or_default() += 1;
        }
        let list: String = counts.iter().map(|(name, n)| format!("{name}[{n}]")).collect();
        match (hurt, counts.is_empty()) {
            (true, true) => say(m.id, 1, "Ow! Whoever did that got away"),
            (true, false) => say(m.id, 1, format!("Caught the culprit on film! {list}")),
            (false, true) => say(m.id, 2, "Click! Nothing worth printing"),
            (false, false) => say(m.id, 1, format!("Scoop! {list}")),
        }
    }
}

impl Goal for AyaShot {
    fn flags(&self) -> u8 {
        0
    }
    fn can_use(&mut self, m: &mut Mob, _: &mut Cx) -> bool {
        m.alive() && state(m.id).is_some_and(|s| s.enabled)
    }
    fn start(&mut self, m: &mut Mob, _: &mut Cx) {
        self.health = m.health;
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        let hurt = m.health < self.health;
        self.health = m.health;
        self.counter += 1;
        self.film += 1;
        // Punched with an empty hand: she turns her camera on whoever did it, and keeps the shot.
        if SNAPPED.lock().is_ok_and(|mut s| s.remove(&m.id)) {
            let aim = cx.w.player.as_ref().map(|p| p.eye);
            if let Some(at) = aim {
                m.look_at(at);
            }
            self.photo(m, cx, false);
            if let Ok(mut p) = PHOTOS.lock() {
                p.push(aya_photo(m, aim));
            }
            return;
        }
        if hurt || self.counter >= AYA_INTERVAL {
            self.counter = 0;
            self.photo(m, cx, hurt);
            // The original saved a screenshot of each: this one keeps one when she's hurt, and one
            // every five minutes she's out with her owner, not to fill the folder.
            let out = in_mode(m, PATROL) || in_mode(m, FOLLOW) || in_mode(m, RIDE);
            if hurt || (out && self.film >= AYA_FILM) {
                self.film = 0;
                if let Ok(mut p) = PHOTOS.lock() {
                    p.push(aya_photo(m, None));
                }
            }
        }
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
    fn name(&self) -> &'static str {
        "AyaShot"
    }
}

/// On patrol, every hostile mob around it; following or on its owner's head, whatever is after its
/// owner or near them. Once a second it closes in (from the head, it doesn't) and swings, up to seven
/// blocks off, throwing what it hits back a block.
#[derive(Clone, Debug, Default)]
struct GoliathAttack {
    /// A player it goes after, when there is no mob to fight.
    player: Option<u128>,
    /// Its blow, when it isn't Goliath's own (`GOLIATH_STRENGTH`).
    strength: f32,
    target: Option<u32>,
    last_scan: i64,
    counter: i32,
    lost: i32,
}

impl Goal for GoliathAttack {
    fn flags(&self) -> u8 {
        goals::MOVE | goals::LOOK
    }
    fn can_use(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        if cx.w.game_time - self.last_scan < 20 {
            return false;
        }
        self.last_scan = cx.w.game_time;
        // Whatever is attacking it comes first.
        self.target = if let Some(a) = fighting_mode(m).then(|| my_attacker(m, cx)).flatten() {
            Some(a)
        } else if in_mode(m, PATROL) {
            nearest(m, cx, |s| enemy(s) || fights_back(m, s))
        } else if in_mode(m, FOLLOW) || in_mode(m, RIDE) {
            nearest(m, cx, |s| {
                (!is_doll(s.id) && (matches!(s.target, Some(Who::Player)) || (enemy(s) && near_owner(m, cx, s))))
                    || fights_back(m, s)
            })
        } else {
            None
        };
        self.player = if self.target.is_none() && fighting_mode(m) { player_foe(m, cx.w).map(|(p, _)| p) } else { None };
        self.target.is_some() || self.player.is_some()
    }
    fn can_continue(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        if fighting_mode(m) && attacker_elsewhere(m, cx, self.target) {
            return false;
        }
        if let Some(p) = self.player {
            return fighting_mode(m) && self.lost < LOST_AFTER && player_foe(m, cx.w).is_some_and(|(q, _)| q == p);
        }
        fighting_mode(m) && self.lost < LOST_AFTER && self.target.and_then(|id| cx.w.seen(id)).is_some_and(|s| s.alive)
    }
    fn start(&mut self, _: &mut Mob, _: &mut Cx) {
        self.counter = 0;
        self.lost = 0;
    }
    fn stop(&mut self, m: &mut Mob, _: &mut Cx) {
        self.target = None;
        self.player = None;
        m.nav_stop();
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        if let Some(p) = self.player {
            let Some((_, o)) = player_foe(m, cx.w).filter(|(q, _)| *q == p) else {
                self.lost += 1;
                return;
            };
            m.look_at(o.eye);
            self.counter -= 1;
            if self.counter > 0 {
                return;
            }
            self.counter = 20;
            if !in_mode(m, RIDE) {
                m.nav_move_to_entity(cx.w, o.position, 0, 1.0);
            }
            if distance_sqr(m, o.position) < GOLIATH_REACH_SQR {
                m.swing();
                mark_swing(m.id);
                let blow = attack(m.id, if self.strength > 0.0 { self.strength } else { GOLIATH_STRENGTH });
                hit_player(m.id, p, blow * goliath_fury(m.health, m.attrs.max_health));
                worked(m.id);
                cx.events.push(MobEvent::Sound("entity.blaze.hurt", o.position, 0.8, 1.0));
            }
            return;
        }
        let Some(id) = self.target else { return };
        let Some((at, eye, alive)) = cx.w.seen(id).map(|s| (pos(s), s.eye_y, s.alive)) else {
            return;
        };
        let sees = m.sees(cx.w, Who::Mob(id));
        if sees {
            m.look_at([at[0], eye, at[2]]);
            self.lost = (self.lost - 1).max(0);
        } else {
            self.lost += 1;
        }
        self.counter -= 1;
        if self.counter > 0 {
            return;
        }
        self.counter = 20;
        // On its owner's head it swings from where it sits.
        if !in_mode(m, RIDE) {
            m.nav_move_to_entity(cx.w, at, 0, 1.0);
        }
        if alive && sees && distance_sqr(m, at) < GOLIATH_REACH_SQR {
            m.swing();
            mark_swing(m.id);
            cx.events.push(MobEvent::AttackMob {
                target: id,
                amount: attack(m.id, if self.strength > 0.0 { self.strength } else { GOLIATH_STRENGTH })
                    * goliath_fury(m.health, m.attrs.max_health),
                source: Damage::player(here(m)),
            });
            goliath_struck(id, m.id);
            worked(m.id);
            let (dx, dz) = (at[0] - m.position.x, at[2] - m.position.z);
            let dr = (dx * dx + dz * dz + 0.01).sqrt();
            cx.events.push(MobEvent::Push { target: Who::Mob(id), velocity: [dx / dr, 0.1, dz / dr] });
            cx.events.push(MobEvent::Sound("entity.blaze.hurt", at, 0.8, 1.0));
        }
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
    fn name(&self) -> &'static str {
        "GoliathAttack"
    }
}

/// A crop that bonemeal grows: its age now and the age it is grown at. Melon and pumpkin stems grow to
/// 7 (their fruit she leaves to the stem); cocoa to 2.
fn bloomable(w: &MobWorld, p: [i32; 3]) -> Option<(u32, u32)> {
    let name = name_at(w, p);
    let max = if let Some(c) = CROPS.iter().find(|c| *c.0 == *name) {
        c.2.parse().ok()?
    } else if is_stem(&name) && !name.starts_with("minecraft:attached") {
        7
    } else if &*name == "minecraft:cocoa" {
        2
    } else {
        return None;
    };
    let [x, y, z] = p;
    let age: u32 = w.registry.state(w.world.block(x, y, z))?.properties.get("age")?.parse().ok()?;
    (age < max).then_some((age, max))
}

fn stem_beside(w: &MobWorld, p: [i32; 3]) -> bool {
    [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]].iter().any(|&d| is_stem(&name_at(w, add(p, d))))
}

/// The plants that grow up from their bottom block, which Yuuka always leaves.
const TALL_PLANTS: [&str; 3] = ["minecraft:cactus", "minecraft:sugar_cane", "minecraft:bamboo"];

/// A crop at its last age, a melon or pumpkin with a stem beside it, or a cactus, sugar cane or
/// bamboo two blocks tall or more.
fn harvestable(w: &MobWorld, p: [i32; 3]) -> Option<&'static str> {
    let name = name_at(w, p);
    if let Some(crop) = CROPS.iter().find(|c| *c.0 == *name) {
        let [x, y, z] = p;
        let state = w.world.block(x, y, z);
        let ripe = w
            .registry
            .state(state)
            .and_then(|s| s.properties.get("age"))
            .is_some_and(|age| age.as_str() == crop.2);
        return ripe.then_some(crop.0);
    }
    if &*name == "minecraft:cocoa" {
        let [x, y, z] = p;
        let ripe = w.registry.state(w.world.block(x, y, z)).and_then(|s| s.properties.get("age")).is_some_and(|a| a.as_str() == "2");
        return ripe.then_some("minecraft:cocoa");
    }
    // Cactus, sugar cane and bamboo: cut from the second block up, so the bottom one stays and
    // grows again (what is above it comes down with it).
    if let Some(tall) = TALL_PLANTS.iter().copied().find(|t| *t == &*name)
        && *name_at(w, add(p, [0, -1, 0])) == *tall
        && *name_at(w, add(p, [0, -2, 0])) != *tall
    {
        return Some(tall);
    }
    match &*name {
        "minecraft:melon" if stem_beside(w, p) => Some("minecraft:melon"),
        "minecraft:pumpkin" if stem_beside(w, p) => Some("minecraft:pumpkin"),
        _ => None,
    }
}

/// Whether Yuuka is at work: on patrol, or in her other mode.
fn farming(m: &Mob) -> bool {
    in_mode(m, PATROL) || in_mode(m, BLOOM)
}

/// On patrol: tills the dirt it stands on between farmland, harvests what is ripe within reach
/// (cocoa pods on their logs too) and plants it straight back, and fills any empty farmland around
/// her with what she last grew.
#[derive(Clone, Debug, Default)]
struct Farmer {
    target: [i32; 3],
    block: Option<&'static str>,
    counter: i32,
    last_scan: i64,
}

impl Farmer {
    /// Ticks between her looks round for work: a second, shorter with favour; in her other mode, less.
    fn last_scan_gap(&self, m: &Mob) -> i32 {
        if in_mode(m, BLOOM) {
            return BLOOM_GAP;
        }
        quicker(20, boost(m.id)).max(4)
    }

    /// Her other mode's round.
    fn bloom(&self, m: &mut Mob, cx: &mut Cx) {
        if !in_mode(m, BLOOM) {
            return;
        }
        if edit_inventory(m.id, |items| bake_bread(items)).unwrap_or(0) > 0 {
            worked(m.id);
        }
        let eye = eye_block(m);
        let r = FARM_RANGE;
        let mut growing: Vec<([i32; 3], u32, u32, i32)> = Vec::new();
        for dy in -r..=r {
            for dx in -r..=r {
                for dz in -r..=r {
                    let d = [dx, dy, dz];
                    let n = steps(d, [0; 3]);
                    if n > r {
                        continue;
                    }
                    if let Some((age, max)) = bloomable(cx.w, add(eye, d)) {
                        growing.push((add(eye, d), age, max, n));
                    }
                }
            }
        }
        growing.sort_by_key(|g| g.3);
        for (p, age, max, _) in growing.into_iter().take(BLOOM_PER_ROUND) {
            let [x, y, z] = p;
            let Some(mut state) = cx.w.registry.state(cx.w.world.block(x, y, z)).cloned() else { continue };
            let roll = m.random.next_int_bound(4).unwrap_or(0);
            state.properties.insert("age".into(), bloom_age(age, max, roll).to_string().into());
            let Ok(state) = cx.w.registry.resolve(&state) else { continue };
            let at = p.map(|c| f64::from(c) + 0.5);
            cx.events.push(MobEvent::SetBlock { pos: p, state });
            cx.events.push(MobEvent::ParticleAt { kind: "happy_villager", at, velocity: [0.0; 3] });
            cx.events.push(MobEvent::Sound("item.bone_meal.use", at, 0.6, 1.0));
            worked(m.id);
        }
    }

    /// Farmland trampled lately near her: back to farmland, with what she last grew planted in it.
    fn retill(&self, m: &mut Mob, cx: &mut Cx) {
        let feet = [m.position.x.floor() as i32, m.position.y.floor() as i32, m.position.z.floor() as i32];
        let w = cx.w;
        for p in trampled_near(feet, world_dimension(w), FARM_RANGE + 1) {
            let name = name_at(w, p);
            let above = add(p, [0, 1, 0]);
            // Not dirt any more (tilled by someone, built on), or something on it: off the list.
            if !matches!(&*name, "minecraft:dirt") || !is_air(&name_at(w, above)) {
                if &*name != "minecraft:farmland" {
                    forget_trampled(p);
                }
                continue;
            }
            forget_trampled(p);
            let at = p.map(|c| f64::from(c) + 0.5);
            m.look_at(at);
            cx.events.push(MobEvent::Sound("item.hoe.till", at, 1.0, 1.0));
            cx.events.push(MobEvent::SetBlockName { mob: Some(m.id), pos: p, name: "minecraft:farmland", effect: false, flags: 3 });
            let last = LAST_CROP.lock().ok().and_then(|l| l.get(&m.id).copied());
            let seeded = holds(m.id, |item| plant_for(item).is_some());
            if let Some(plant) = last.or_else(|| seeded.as_deref().and_then(plant_for))
                && !stem_beside(w, above)
            {
                if let Some(seed) = CROPS.iter().find(|c| c.0 == plant).map(|c| c.1).or_else(|| STEMS.iter().find(|s| s.0 == plant).map(|s| s.1)) {
                    take_one(m.id, seed);
                }
                cx.events.push(MobEvent::SetBlockName { mob: Some(m.id), pos: above, name: plant, effect: false, flags: 3 });
            }
            worked(m.id);
            m.swing();
            mark_swing(m.id);
            // One patch a look round, as she tills her own.
            break;
        }
    }

    fn till_and_plant(&self, m: &mut Mob, cx: &mut Cx) {
        self.retill(m, cx);
        self.fill_farmland(m, cx);
        let w = cx.w;
        let [x, y, z] = eye_block(m);
        for j in 0..FARM_RANGE {
            let p = [x, y - j, z];
            let name = name_at(w, p);
            if !matches!(&*name, "minecraft:dirt" | "minecraft:grass_block") {
                continue;
            }
            let farmland = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]]
                .iter()
                .filter(|&&d| &*name_at(w, add(p, d)) == "minecraft:farmland")
                .count();
            if farmland >= 2 && is_air(&name_at(w, add(p, [0, 1, 0]))) {
                let at = p.map(|c| f64::from(c) + 0.5);
                cx.events.push(MobEvent::Sound("item.hoe.till", at, 1.0, 1.0));
                cx.events.push(MobEvent::SetBlockName {
                    mob: Some(m.id),
                    pos: p,
                    name: "minecraft:farmland",
                    effect: false,
                    flags: 3,
                });
                worked(m.id);
                m.swing();
                mark_swing(m.id);
                break;
            }
        }
        for j in 0..FARM_RANGE {
            let p = [x, y - j, z];
            let above = add(p, [0, 1, 0]);
            if &*name_at(w, p) != "minecraft:farmland" || !is_air(&name_at(w, above)) {
                continue;
            }
            if stem_beside(w, above) {
                break;
            }
            // What she last grew, else whatever seed she carries.
            let last = LAST_CROP.lock().ok().and_then(|l| l.get(&m.id).copied());
            let seeded = holds(m.id, |item| plant_for(item).is_some());
            let plant = last.or_else(|| seeded.as_deref().and_then(plant_for));
            if let Some(plant) = plant {
                if let Some(seed) = CROPS.iter().find(|c| c.0 == plant).map(|c| c.1).or_else(|| STEMS.iter().find(|s| s.0 == plant).map(|s| s.1)) {
                    take_one(m.id, seed);
                }
                if let Ok(mut l) = LAST_CROP.lock() {
                    l.insert(m.id, plant);
                }
                cx.events.push(MobEvent::SetBlockName {
                    mob: Some(m.id),
                    pos: above,
                    name: plant,
                    effect: false,
                    flags: 3,
                });
                worked(m.id);
                m.swing();
                mark_swing(m.id);
            }
            break;
        }
    }
}

impl Goal for Farmer {
    fn flags(&self) -> u8 {
        goals::MOVE | goals::LOOK
    }
    fn can_use(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        if !farming(m) || cx.w.game_time - self.last_scan < i64::from(self.last_scan_gap(m)) {
            return false;
        }
        self.last_scan = cx.w.game_time;
        self.bloom(m, cx);
        self.till_and_plant(m, cx);
        let eye = eye_block(m);
        let r = FARM_RANGE;
        let mut best: Option<([i32; 3], &'static str, i32)> = None;
        for dy in -r..=r {
            for dx in -r..=r {
                for dz in -r..=r {
                    let d = [dx, dy, dz];
                    let n = steps(d, [0; 3]);
                    if n > r || best.is_some_and(|b| b.2 <= n) {
                        continue;
                    }
                    if let Some(block) = harvestable(cx.w, add(eye, d)) {
                        best = Some((add(eye, d), block, n));
                    }
                }
            }
        }
        let Some((at, block, _)) = best else { return false };
        self.target = at;
        self.block = Some(block);
        true
    }
    fn can_continue(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        let at = [m.position.x.floor() as i32, m.position.y.floor() as i32, m.position.z.floor() as i32];
        farming(m)
            && steps(at, self.target) <= FARM_RANGE + 1
            && self.block.is_some_and(|b| *name_at(cx.w, self.target) == *b)
    }
    fn start(&mut self, _: &mut Mob, _: &mut Cx) {
        self.counter = 0;
    }
    fn stop(&mut self, m: &mut Mob, _: &mut Cx) {
        self.block = None;
        m.nav_stop();
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        let Some(block) = self.block else { return };
        let at = self.target.map(|c| f64::from(c) + 0.5);
        m.look_at(at);
        if self.counter == 0 {
            m.nav_move_to(cx.w, at, 1.0);
        }
        if self.counter == self.last_scan_gap(m) {
            // A cocoa pod is put back as a new one, on the same side of its log.
            let new_pod = (block == "minecraft:cocoa")
                .then(|| {
                    let [x, y, z] = self.target;
                    let mut state = cx.w.registry.state(cx.w.world.block(x, y, z))?.clone();
                    state.properties.insert("age".into(), "0".into());
                    cx.w.registry.resolve(&state).ok()
                })
                .flatten();
            // A pumpkin comes off with Silk Touch: the block itself, whatever its loot says.
            let silk = block == "minecraft:pumpkin";
            cx.events.push(MobEvent::BreakBlock { pos: self.target, drop: !silk, by: None, breaker: Some(m.id) });
            if silk {
                cx.events.push(MobEvent::DropItem { mob: m.id, item: Arc::from(block), count: 1, at });
            }
            worked(m.id);
            if let Some(state) = new_pod {
                take_one(m.id, "minecraft:cocoa_beans");
                cx.events.push(MobEvent::SetBlock { pos: self.target, state });
            }
            m.swing();
            mark_swing(m.id);
            // A harvested crop goes straight back in, whatever it was, from its seeds if she has them.
            if let Some(crop) = CROPS.iter().find(|c| c.0 == block) {
                take_one(m.id, crop.1);
                if let Ok(mut l) = LAST_CROP.lock() {
                    l.insert(m.id, crop.0);
                }
                cx.events.push(MobEvent::SetBlockName {
                    mob: Some(m.id),
                    pos: self.target,
                    name: crop.0,
                    effect: false,
                    flags: 3,
                });
            }
            self.block = None;
        }
        self.counter += 1;
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
    fn name(&self) -> &'static str {
        "Farmer"
    }
}

// ---- The Diamond and Netherite Horai: Efficiency V and Silk Touch, strip-mining from the head, the
// Netherite one's quarry, and the hunt for diamonds and ancient debris ----

/// 0 the Horai doll, 1 the Diamond Horai, 2 the Netherite Horai.
fn horai_tier(m: &Mob) -> u8 {
    match m.custom_name.as_deref() {
        Some(DIAMOND_HORAI) => 1,
        Some(NETHERITE_HORAI) => 2,
        _ => 0,
    }
}

/// Breaks `p` as a pickaxe with Silk Touch: an ore (or ancient debris) drops itself at `to`, but
/// coal and iron ore drop coal and raw iron with Fortune III; anything else its usual drops where it
/// was, or nothing when it is filler and `filler_drops` is false. Every block counts to its love.
fn break_out(m: &mut Mob, cx: &mut Cx, p: [i32; 3], to: [f64; 3], filler_drops: bool) {
    let name = name_at(cx.w, p);
    if let Some(drop) = fortune_drop(&name) {
        let roll = m.random.next_int_bound(FORTUNE + 2).unwrap_or(0);
        cx.events.push(MobEvent::BreakBlock { pos: p, drop: false, by: None, breaker: Some(m.id) });
        cx.events.push(MobEvent::DropItem { mob: m.id, item: Arc::from(drop), count: fortune_count(roll), at: to });
    } else if is_ore(&name) || is_treasure(&name) {
        cx.events.push(MobEvent::BreakBlock { pos: p, drop: false, by: None, breaker: Some(m.id) });
        cx.events.push(MobEvent::DropItem { mob: m.id, item: name, count: 1, at: to });
    } else {
        cx.events.push(MobEvent::BreakBlock { pos: p, drop: filler_drops || !is_filler(&name), by: None, breaker: Some(m.id) });
    }
    worked(m.id);
}

/// What a quarry or a tunnel may dig: solid, no harder than obsidian, not bedrock, a liquid, or her
/// own scaffolding.
fn can_quarry(w: &MobWorld, p: [i32; 3]) -> bool {
    if is_placed(p) {
        return false;
    }
    let name = name_at(w, p);
    if is_doll_block(&name) {
        return false;
    }
    if is_air(&name) || is_liquid(&name) || matches!(&*name, "minecraft:bedrock" | PHANTOM_SCAFFOLDING) {
        return false;
    }
    (0.0..=HARDEST).contains(&hardness_at(w, p))
}

/// The first lava in the box from `lo` to `hi`, both included.
fn lava_in_box(w: &MobWorld, lo: [i32; 3], hi: [i32; 3]) -> Option<[i32; 3]> {
    for y in lo[1]..=hi[1] {
        for x in lo[0]..=hi[0] {
            for z in lo[2]..=hi[2] {
                if &*name_at(w, [x, y, z]) == "minecraft:lava" {
                    return Some([x, y, z]);
                }
            }
        }
    }
    None
}

/// Lava within `LAVA_ACROSS` across and `LAVA_UP` up or down of `p`.
fn lava_close(w: &MobWorld, p: [i32; 3]) -> Option<[i32; 3]> {
    lava_in_box(w, add(p, [-LAVA_ACROSS, -LAVA_UP, -LAVA_ACROSS]), add(p, [LAVA_ACROSS, LAVA_UP, LAVA_ACROSS]))
}

/// One block being dug, with the cracks and sounds of it.
#[derive(Clone, Debug, Default)]
struct Dig {
    at: Option<[i32; 3]>,
    counter: i32,
    total: i32,
}

impl Dig {
    fn begin(&mut self, w: &MobWorld, tier: u8, p: [i32; 3], boost: f64) {
        let c = quicker(dig_ticks(tier, hardness_at(w, p)), boost);
        self.at = Some(p);
        self.counter = c;
        self.total = c.max(1);
    }
    /// One tick of digging: the block, when it is ready to come out.
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) -> Option<[i32; 3]> {
        let p = self.at?;
        let at = centre(p);
        m.look_at(at);
        if is_air(&name_at(cx.w, p)) {
            self.cancel(m, cx);
            return None;
        }
        if self.counter % 6 == 0 {
            m.swing();
            mark_swing(m.id);
        }
        if self.counter <= 0 {
            self.at = None;
            cx.events.push(MobEvent::DestroyProgress { mob: m.id, pos: p, progress: -1 });
            return Some(p);
        }
        let progress = ((self.total - self.counter) * 10 / self.total).min(9);
        cx.events.push(MobEvent::DestroyProgress { mob: m.id, pos: p, progress });
        if self.counter % 4 == 0 {
            let facts = cx.w.facts.get(cx.w.world.block(p[0], p[1], p[2]));
            let sound = facts.sounds.break_sound.to_string();
            let volume = (facts.sounds.volume + 1.0) / 8.0;
            let pitch = facts.sounds.pitch * 0.5;
            cx.events.push(MobEvent::SoundId(sound, at, volume, pitch));
        }
        self.counter -= 1;
        None
    }
    fn cancel(&mut self, m: &Mob, cx: &mut Cx) {
        if let Some(p) = self.at.take() {
            cx.events.push(MobEvent::DestroyProgress { mob: m.id, pos: p, progress: -1 });
        }
    }
}

/// Holds a digging doll in the cell `feet`, floating: she goes where she digs, not where she falls.
fn hold_in(m: &mut Mob, feet: [f64; 3]) {
    m.position.x = feet[0];
    m.position.y = feet[1];
    m.position.z = feet[2];
    m.velocity = horizon_core::geometry::Vec3::new(0.0, 0.0, 0.0);
    m.no_gravity = true;
    m.fall_distance = 0.0;
    m.nav_stop();
}

fn move_to_cell(m: &mut Mob, p: [i32; 3]) {
    hold_in(m, [f64::from(p[0]) + 0.5, f64::from(p[1]), f64::from(p[2]) + 0.5]);
    m.previous = m.position;
}

/// On her owner's head, each block they break straight ahead of them, the Diamond or Netherite Horai
/// digs on: the rest of that column of their two-high tunnel, and the two columns after it.
#[derive(Clone, Debug, Default)]
struct StripMine {
    seen: u64,
    queue: Vec<[i32; 3]>,
    dig: Dig,
}

impl StripMine {
    /// Takes in a block the player broke since the last look, and queues the tunnel past it.
    fn take_dug(&mut self, m: &Mob, cx: &Cx) {
        let (n, dug) = owner_dug(m.id);
        if n == self.seen {
            return;
        }
        let first = self.seen == 0;
        self.seen = n;
        if first {
            return;
        }
        let Some(p) = owner_seen(m, cx.w) else { return };
        let feet = p.position.map(|c| c.floor() as i32);
        let dir = if p.look[0].abs() > p.look[2].abs() {
            [p.look[0].signum() as i32, 0, 0]
        } else {
            [0, 0, p.look[2].signum() as i32]
        };
        // Straight ahead of them, at their feet or their head, within reach.
        let ahead = (dug[0] - feet[0]) * dir[0] + (dug[2] - feet[2]) * dir[2];
        let aside = (dug[0] - feet[0]) * dir[2] - (dug[2] - feet[2]) * dir[0];
        if !(1..=5).contains(&ahead) || aside != 0 || !(feet[1]..=feet[1] + 1).contains(&dug[1]) {
            return;
        }
        'tunnel: for k in 0..=2 {
            for y in [feet[1], feet[1] + 1] {
                let cell = [dug[0] + dir[0] * k, y, dug[2] + dir[2] * k];
                if cell == dug || self.queue.contains(&cell) {
                    continue;
                }
                // Never opens onto lava: the tunnel stops short of it.
                if SIDES.iter().any(|&d| &*name_at(cx.w, add(cell, d)) == "minecraft:lava") {
                    break 'tunnel;
                }
                self.queue.push(cell);
            }
        }
    }
}

impl Goal for StripMine {
    fn flags(&self) -> u8 {
        goals::LOOK
    }
    fn can_use(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        if horai_tier(m) == 0 || !in_mode(m, RIDE) {
            self.seen = owner_dug(m.id).0.max(1);
            return false;
        }
        self.take_dug(m, cx);
        !self.queue.is_empty()
    }
    fn can_continue(&mut self, m: &mut Mob, _: &mut Cx) -> bool {
        in_mode(m, RIDE) && (self.dig.at.is_some() || !self.queue.is_empty())
    }
    fn stop(&mut self, m: &mut Mob, cx: &mut Cx) {
        self.dig.cancel(m, cx);
        self.queue.clear();
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        self.take_dug(m, cx);
        if self.dig.at.is_some() {
            if let Some(p) = self.dig.tick(m, cx) {
                break_out(m, cx, p, centre(p), true);
            }
            return;
        }
        while !self.queue.is_empty() {
            let p = self.queue.remove(0);
            if can_quarry(cx.w, p) {
                self.dig.begin(cx.w, horai_tier(m), p, boost(m.id));
                return;
            }
        }
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
    fn name(&self) -> &'static str {
        "StripMine"
    }
}

/// The top solid block of column `x z`, a few blocks either side of `y`.
fn ground(w: &MobWorld, x: i32, y: i32, z: i32) -> Option<i32> {
    (y - 6..=y + 3).rev().find(|&y| {
        let below = w.facts.get(w.world.block(x, y, z));
        let above = name_at(w, [x, y + 1, z]);
        below.full_collision && !is_liquid(&below.name) && (is_air(&above) || !w.facts.get(w.world.block(x, y + 1, z)).full_collision) && !is_liquid(&above)
    })
}

#[derive(Clone, Debug, Default)]
struct Quarry {
    /// The shaft's middle column, at the ground block it starts from.
    site: Option<[i32; 3]>,
    /// The layer being dug; once its cells are done, the layer last dug out.
    layer: i32,
    cells: Vec<[i32; 3]>,
    vein: Vec<[i32; 3]>,
    queued: HashSet<[i32; 3]>,
    /// The cell she stands in while she digs out a vein.
    stand: Option<[i32; 3]>,
    dig: Dig,
    /// Lava found, which no new shaft comes within ten blocks of.
    lava: Vec<[i32; 3]>,
    retries: i32,
    wait: i32,
    /// Holes to cover once their scaffolding column is in: a column cell, the cells to cover, and
    /// ticks left to wait for it.
    covers: Vec<([i32; 3], Vec<[i32; 3]>, i32)>,
    fetch: Fetch,
}

/// The phantom scaffolding's `cover=true` state, solid to stand on: learned from a column placed in
/// its default state (`cover=false`, climbed through), the one after it.
fn cover_state(w: &MobWorld, column: [i32; 3]) -> Option<horizon_world::registry::BlockStateId> {
    let here = w.world.block(column[0], column[1], column[2]);
    if &*w.facts.get(here).name != PHANTOM_SCAFFOLDING {
        return None;
    }
    let next = horizon_world::registry::BlockStateId(here.0 + 1);
    (&*w.facts.get(next).name == PHANTOM_SCAFFOLDING).then_some(next)
}

impl Quarry {
    fn hold(&self, m: &mut Mob) {
        if let Some(s) = self.stand {
            hold_in(m, [f64::from(s[0]) + 0.5, f64::from(s[1]), f64::from(s[2]) + 0.5]);
        } else if let Some(site) = self.site {
            // On top of the layer she digs.
            hold_in(m, [f64::from(site[0]) + 0.5, f64::from(self.layer + 1), f64::from(site[2]) + 0.5]);
        }
    }

    fn found_lava(&mut self, l: [i32; 3]) {
        self.lava.push(l);
        if self.lava.len() > 32 {
            self.lava.remove(0);
        }
    }

    /// A new shaft behind her owner, or as near behind them as is clear of lava; she teleports there.
    fn new_site(&mut self, m: &mut Mob, cx: &mut Cx) {
        let Some(p) = owner_seen(m, cx.w) else { return };
        let [px, py, pz] = p.position;
        let n = (p.look[0] * p.look[0] + p.look[2] * p.look[2]).sqrt();
        let (bx, bz) = if n > 1e-3 { (-p.look[0] / n, -p.look[2] / n) } else { (0.0, 1.0) };
        for dist in [3.0, 6.0, 10.0, 14.0] {
            for turn in [0.0f64, 45.0, -45.0, 90.0, -90.0, 135.0, -135.0, 180.0] {
                let (s, c) = turn.to_radians().sin_cos();
                let x = (px + (bx * c - bz * s) * dist).floor() as i32;
                let z = (pz + (bx * s + bz * c) * dist).floor() as i32;
                if self.lava.iter().any(|l| (l[0] - x).abs() <= 1 + LAVA_ACROSS + 1 && (l[2] - z).abs() <= 1 + LAVA_ACROSS + 1) {
                    continue;
                }
                let Some(top) = ground(cx.w, x, py.floor() as i32, z) else { continue };
                let reach = 1 + LAVA_ACROSS;
                if let Some(l) = lava_in_box(cx.w, [x - reach, top - LAVA_UP, z - reach], [x + reach, top + 3, z + reach]) {
                    self.found_lava(l);
                    continue;
                }
                self.site = Some([x, top, z]);
                self.layer = top + 1;
                self.cells.clear();
                self.vein.clear();
                self.queued.clear();
                self.stand = None;
                self.retries = 0;
                move_to_cell(m, [x, top + 1, z]);
                cx.events.push(MobEvent::Sound("entity.enderman.teleport", here(m), 1.0, 1.0));
                say(m.id, 2, "Quarrying here");
                return;
            }
        }
        say(m.id, 1, "Nowhere safe from lava to dig here");
        self.wait = 100;
    }

    /// Done with this shaft: scaffolding up one corner, from its floor to the top, to climb out by,
    /// and the rest of the top covered over with it, to walk on.
    fn finish(&mut self, m: &mut Mob, cx: &mut Cx) {
        self.dig.cancel(m, cx);
        if let Some(site) = self.site.take() {
            let (x, z) = (site[0] - 1, site[2] - 1);
            let mut placed = false;
            let mut last = None;
            for y in self.layer..=site[1] {
                let p = [x, y, z];
                if !is_air(&name_at(cx.w, p)) {
                    if placed {
                        break;
                    }
                    continue;
                }
                cx.events.push(MobEvent::SetBlockName {
                    mob: Some(m.id),
                    pos: p,
                    name: PHANTOM_SCAFFOLDING,
                    effect: false,
                    flags: 3,
                });
                placed = true;
                last = Some(p);
            }
            if let Some(column) = last
                && self.layer <= site[1]
            {
                let cells: Vec<[i32; 3]> = layer_cells(site, site[1])
                    .into_iter()
                    .filter(|&p| p != [x, site[1], z] && is_air(&name_at(cx.w, p)))
                    .collect();
                self.covers.push((column, cells, 40));
            }
        }
        self.cells.clear();
        self.vein.clear();
        self.queued.clear();
        self.stand = None;
        self.wait = 20;
    }

    /// Ore beside a block just dug: queued, with its whole vein in turn, unless it is lava's
    /// neighbour or the shaft will dig it anyway.
    fn look_for_vein(&mut self, w: &MobWorld, p: [i32; 3]) {
        let Some(site) = self.site else { return };
        for d in SIDES {
            let q = add(p, d);
            let in_shaft = (q[0] - site[0]).abs() <= 1 && (q[2] - site[2]).abs() <= 1 && q[1] <= site[1];
            if in_shaft || (q[0] - site[0]).abs() > 12 || (q[2] - site[2]).abs() > 12 || self.queued.len() > 4096 {
                continue;
            }
            if is_ore(&name_at(w, q)) && self.queued.insert(q) {
                self.vein.push(q);
            }
        }
    }
}

impl Goal for Quarry {
    fn flags(&self) -> u8 {
        goals::MOVE | goals::LOOK
    }
    fn can_use(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        horai_tier(m) == 2 && in_mode(m, PATROL) && owner_position(m, cx).is_some()
    }
    fn can_continue(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        self.can_use(m, cx)
    }
    fn start(&mut self, _: &mut Mob, _: &mut Cx) {
        // Each patrol starts a new shaft behind the owner, four seconds on: time to click on through
        // to Follow or Rideon before she's gone.
        self.site = None;
        self.wait = QUARRY_DELAY;
    }
    fn stop(&mut self, m: &mut Mob, cx: &mut Cx) {
        self.dig.cancel(m, cx);
        self.stand = None;
    }
    fn every_tick(&self) -> bool {
        true
    }
    /// Twice a tick: she quarries twice as fast as she digs anywhere else.
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        self.step(m, cx);
        if self.site.is_some() && self.fetch.back.is_none() {
            self.step(m, cx);
        }
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
    fn name(&self) -> &'static str {
        "Quarry"
    }
}

impl Quarry {
    fn step(&mut self, m: &mut Mob, cx: &mut Cx) {
        // Covers waiting on their column, which is in the world a tick after it was placed.
        let w = cx.w;
        let events = &mut cx.events;
        self.covers.retain_mut(|(column, cells, left)| {
            *left -= 1;
            let Some(cover) = cover_state(w, *column) else { return *left > 0 };
            for &p in cells.iter() {
                if is_air(&name_at(w, p)) {
                    events.push(MobEvent::SetBlock { pos: p, state: cover });
                }
            }
            false
        });
        if self.wait > 0 {
            self.wait -= 1;
            return;
        }
        let Some(site) = self.site else {
            self.new_site(m, cx);
            return;
        };
        if self.dig.at.is_none() && self.fetch.tick(m, cx) {
            return;
        }
        self.hold(m);
        if self.dig.at.is_some() {
            if let Some(p) = self.dig.tick(m, cx) {
                if is_diamond(&name_at(cx.w, p)) {
                    prize(m.id);
                }
                break_out(m, cx, p, centre(p), keep_rock(m.id));
                self.look_for_vein(cx.w, p);
                // Into the ore's place, to reach the next of its vein.
                if self.stand.is_some() {
                    self.stand = Some(p);
                }
            }
            return;
        }
        // A vein first, in the order it was found; then back to the shaft.
        while !self.vein.is_empty() {
            let p = self.vein.remove(0);
            if !is_ore(&name_at(cx.w, p)) || !can_quarry(cx.w, p) {
                continue;
            }
            if let Some(l) = lava_close(cx.w, p) {
                self.found_lava(l);
                continue;
            }
            // From an open cell beside it: the shaft, or the ore before it.
            let open = SIDES.iter().map(|&d| add(p, d)).find(|&q| is_air(&name_at(cx.w, q)));
            if let Some(q) = open {
                self.stand = Some(q);
                move_to_cell(m, q);
            }
            self.dig.begin(cx.w, 2, p, boost(m.id));
            return;
        }
        if self.stand.take().is_some() {
            self.hold(m);
            m.previous = m.position;
        }
        while let Some(p) = self.cells.pop() {
            if can_quarry(cx.w, p) {
                self.dig.begin(cx.w, 2, p, boost(m.id));
                return;
            }
        }
        // This layer done: whatever fell in is dug again, a few times over.
        let left: Vec<[i32; 3]> = layer_cells(site, self.layer).into_iter().filter(|&p| can_quarry(cx.w, p)).collect();
        if !left.is_empty() && self.retries < 3 && self.layer <= site[1] {
            self.retries += 1;
            self.cells = left;
            return;
        }
        self.retries = 0;
        let next = self.layer - 1;
        let bedrock = next <= WORLD_BOTTOM
            || layer_cells(site, next).iter().any(|&p| &*name_at(cx.w, p) == "minecraft:bedrock");
        if bedrock {
            say(m.id, 1, "Bedrock! Coming back");
            quarried(m.id, false);
            self.finish(m, cx);
            return;
        }
        let lo = [site[0] - 1 - LAVA_ACROSS, next - LAVA_UP, site[2] - 1 - LAVA_ACROSS];
        let hi = [site[0] + 1 + LAVA_ACROSS, next + LAVA_UP, site[2] + 1 + LAVA_ACROSS];
        if let Some(l) = lava_in_box(cx.w, lo, hi) {
            self.found_lava(l);
            say(m.id, 1, "Lava ahead! Digging somewhere else");
            self.finish(m, cx);
            return;
        }
        self.layer = next;
        self.cells = layer_cells(site, next);
    }
}

/// Lava or water beside `p` (or above it): digging it would let the liquid in.
fn by_liquid(w: &MobWorld, p: [i32; 3]) -> bool {
    SIDES.iter().any(|&d| is_liquid(&name_at(w, add(p, d))))
}

#[derive(Clone, Debug, Default)]
struct GigaQuarry {
    layer: Option<i32>,
    cells: Vec<[i32; 3]>,
    /// Blocks of this layer dug or left, to tell a layer of bedrock.
    hard: usize,
    dig: Dig,
    fetch: Fetch,
    done: bool,
    /// The pit she is digging, for its ladders once it ends.
    site: Option<[i32; 3]>,
}

/// The ladders down a giga quarry at `site`, dug as deep as layer `bottom`: in the corner where
/// she began, against the pit's west wall, wherever that cell is open and the wall is solid.
fn giga_ladders(w: &MobWorld, site: [i32; 3], bottom: i32) -> Vec<(i32, horizon_world::registry::BlockStateId)> {
    let Some(mut ladder) = w.registry.default_state("minecraft:ladder").and_then(|d| w.registry.state(d).cloned()) else {
        return Vec::new();
    };
    ladder.properties.insert("facing".into(), "east".into());
    let Ok(state) = w.registry.resolve(&ladder) else { return Vec::new() };
    let [x, _, z] = site;
    (bottom.max(WORLD_BOTTOM)..=site[1])
        .rev()
        .filter(|&y| is_air(&name_at(w, [x, y, z])) && w.facts.get(w.world.block(x - 1, y, z)).full_collision)
        .map(|y| (y, state))
        .collect()
}

impl GigaQuarry {
    /// The pit's ladders, from its top layer down to the deepest she dug.
    fn place_ladders(&self, cx: &mut Cx) {
        let (Some(site), Some(bottom)) = (self.site, self.layer) else { return };
        for (y, state) in giga_ladders(cx.w, site, bottom) {
            cx.events.push(MobEvent::SetBlock { pos: [site[0], y, site[2]], state });
        }
    }
}

impl Goal for GigaQuarry {
    fn flags(&self) -> u8 {
        goals::MOVE | goals::LOOK
    }
    fn can_use(&mut self, m: &mut Mob, _: &mut Cx) -> bool {
        in_mode(m, GIGA) && giga_site(m.id).is_some()
    }
    fn can_continue(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        self.can_use(m, cx)
    }
    fn start(&mut self, m: &mut Mob, _: &mut Cx) {
        self.layer = None;
        self.cells.clear();
        self.done = false;
        say(m.id, 1, "Giga quarry: digging a 20x20 pit to bedrock");
    }
    fn stop(&mut self, m: &mut Mob, cx: &mut Cx) {
        self.dig.cancel(m, cx);
        m.no_gravity = false;
        // Ended by another order (not just put aside for a moment): its ladders, as if finished.
        if !in_mode(m, GIGA) && !self.done {
            self.place_ladders(cx);
        }
        self.site = None;
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        self.step(m, cx);
        if horai_tier(m) == 2 && self.fetch.back.is_none() {
            self.step(m, cx);
        }
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
    fn name(&self) -> &'static str {
        "GigaQuarry"
    }
}

impl GigaQuarry {
    fn step(&mut self, m: &mut Mob, cx: &mut Cx) {
        let Some(site) = giga_site(m.id) else { return };
        self.site = Some(site);
        if self.done {
            m.no_gravity = false;
            return;
        }
        if self.dig.at.is_none() && self.fetch.tick(m, cx) {
            return;
        }
        let layer = *self.layer.get_or_insert_with(|| {
            self.cells = giga_cells(site, site[1]);
            self.hard = 0;
            site[1]
        });
        if let Some(p) = self.dig.at {
            hold_in(m, [f64::from(p[0]) + 0.5, f64::from(layer + 1), f64::from(p[2]) + 0.5]);
            if let Some(p) = self.dig.tick(m, cx) {
                if horai_tier(m) == 2 && is_diamond(&name_at(cx.w, p)) {
                    prize(m.id);
                }
                break_out(m, cx, p, centre(p), keep_rock(m.id));
            }
            return;
        }
        let tier = horai_tier(m);
        while let Some(p) = self.cells.pop() {
            let name = name_at(cx.w, p);
            if &*name == "minecraft:bedrock" {
                self.hard += 1;
                continue;
            }
            if !can_quarry(cx.w, p) || by_liquid(cx.w, p) {
                continue;
            }
            move_to_cell(m, [p[0], layer + 1, p[2]]);
            self.dig.begin(cx.w, tier, p, boost(m.id));
            return;
        }
        // The layer is done. Half of it bedrock, or the bottom of the world: the pit is finished.
        let next = layer - 1;
        if next < WORLD_BOTTOM || self.hard * 2 >= (GIGA_SIZE * GIGA_SIZE) as usize {
            self.done = true;
            self.place_ladders(cx);
            say(m.id, 1, "Giga quarry done: bedrock!");
            quarried(m.id, true);
            return;
        }
        self.layer = Some(next);
        self.cells = giga_cells(site, next);
        self.hard = 0;
    }
}

/// The ore hunt: the Diamond or Netherite Horai tunnels to the nearest diamonds and ancient debris,
/// never within seven blocks of lava, and digs them out, Silk Touch and all.
#[derive(Clone, Debug, Default)]
struct SeekOre {
    target: Option<[i32; 3]>,
    /// The cell she is in, on her way.
    at: Option<[i32; 3]>,
    dig: Dig,
    /// Finds she could not get to.
    bad: Vec<[i32; 3]>,
    last_scan: i64,
    step_in: i32,
    told: bool,
    fetch: Fetch,
}

impl SeekOre {
    fn find(&mut self, m: &Mob, cx: &Cx) -> Option<[i32; 3]> {
        let me = [m.position.x.floor() as i32, m.position.y.floor() as i32, m.position.z.floor() as i32];
        let owner = owner_position(m, cx)?;
        let mut found = Vec::new();
        let (across, up, reach) = if horai_tier(m) >= 2 {
            (SEEK_ACROSS_NETHERITE, SEEK_UP_NETHERITE, SEEK_FROM_OWNER_NETHERITE)
        } else {
            (SEEK_ACROSS, SEEK_UP, SEEK_FROM_OWNER)
        };
        for dy in -up..=up {
            for dx in -across..=across {
                for dz in -across..=across {
                    let p = add(me, [dx, dy, dz]);
                    let name = name_at(cx.w, p);
                    if seeks(m, &name) && !self.bad.contains(&p) {
                        found.push((ore_rank(&name), p));
                    }
                }
            }
        }
        // The best ores first, the nearest of them first: ancient debris, diamonds, gold and
        // emeralds; then every other ore; coal last.
        found.sort_by_key(|&(rank, p)| (rank, steps(p, me)));
        for (_, p) in found {
            let c = centre(p);
            let (dx, dy, dz) = (c[0] - owner[0], c[1] - owner[1], c[2] - owner[2]);
            if dx * dx + dy * dy + dz * dz > reach * reach {
                continue;
            }
            if lava_close(cx.w, p).is_some() {
                self.bad.push(p);
                continue;
            }
            return Some(p);
        }
        None
    }

    /// Nothing to dig: she keeps by her owner.
    fn follow(&mut self, m: &mut Mob, cx: &mut Cx) {
        self.at = None;
        let Some(at) = owner_position(m, cx) else { return };
        if distance_sqr(m, at) >= TELEPORT_FROM * TELEPORT_FROM {
            teleport_near(m, cx, at);
        } else if distance_sqr(m, at) > 9.0 && cx.w.game_time % 10 == 0 {
            m.nav_move_to(cx.w, at, 1.0);
        }
    }
}

impl Goal for SeekOre {
    fn flags(&self) -> u8 {
        goals::MOVE | goals::LOOK
    }
    fn can_use(&mut self, m: &mut Mob, _: &mut Cx) -> bool {
        horai_tier(m) >= 1 && secret(m)
    }
    fn can_continue(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        self.can_use(m, cx)
    }
    fn start(&mut self, _: &mut Mob, _: &mut Cx) {
        self.target = None;
        self.at = None;
        self.bad.clear();
        self.last_scan = 0;
        self.told = false;
    }
    fn stop(&mut self, m: &mut Mob, cx: &mut Cx) {
        self.dig.cancel(m, cx);
        self.target = None;
        self.at = None;
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        let tier = horai_tier(m);
        if self.dig.at.is_none() && self.fetch.tick(m, cx) {
            return;
        }
        if let Some(at) = self.at {
            hold_in(m, [f64::from(at[0]) + 0.5, f64::from(at[1]), f64::from(at[2]) + 0.5]);
        }
        if self.dig.at.is_some() {
            if let Some(p) = self.dig.tick(m, cx) {
                let name = name_at(cx.w, p);
                if seeks(m, &name) {
                    if is_treasure(&name) {
                        prize(m.id);
                    }
                    break_out(m, cx, p, centre(p), keep_rock(m.id));
                    say(m.id, 1, format!("Got {}!", short(&name)));
                } else {
                    break_out(m, cx, p, centre(p), keep_rock(m.id));
                }
                // She steps into what she dug.
                self.at = Some(p);
                move_to_cell(m, p);
            }
            return;
        }
        if self.target.is_some_and(|t| !seeks(m, &name_at(cx.w, t))) {
            self.target = None;
        }
        if self.target.is_none() {
            if cx.w.game_time - self.last_scan < 40 {
                self.follow(m, cx);
                return;
            }
            self.last_scan = cx.w.game_time;
            self.target = self.find(m, cx);
            let Some(t) = self.target else {
                if !self.told {
                    let what = if in_mode(m, SECRET_ALL) { "No ore" } else { "No diamonds or ancient debris" };
                    say(m.id, 1, format!("{what} near here"));
                    self.told = true;
                }
                self.follow(m, cx);
                return;
            };
            self.told = false;
            say(m.id, 1, format!("Found {} at {} {} {}", short(&name_at(cx.w, t)), t[0], t[1], t[2]));
            let me = [m.position.x.floor() as i32, m.position.y.floor() as i32, m.position.z.floor() as i32];
            self.at = Some(me);
        }
        let (Some(t), Some(at)) = (self.target, self.at) else { return };
        if steps(at, t) <= 1 {
            self.dig.begin(cx.w, tier, t, boost(m.id));
            return;
        }
        self.step_in -= 1;
        if self.step_in > 0 {
            return;
        }
        self.step_in = 2;
        // A step that brings her closer: through open space if there is one, else dug out. Never
        // into, or right beside, lava or water.
        let mut options: Vec<[i32; 3]> = SIDES.iter().map(|&d| add(at, d)).filter(|&q| steps(q, t) < steps(at, t)).collect();
        options.sort_by_key(|&q| !is_air(&name_at(cx.w, q)));
        for q in options {
            if SIDES.iter().chain(std::iter::once(&[0, 0, 0])).any(|&d| is_liquid(&name_at(cx.w, add(q, d)))) {
                continue;
            }
            if lava_close(cx.w, q).is_some() {
                continue;
            }
            let name = name_at(cx.w, q);
            if is_air(&name) || !cx.w.facts.get(cx.w.world.block(q[0], q[1], q[2])).full_collision {
                self.at = Some(q);
                move_to_cell(m, q);
                return;
            }
            if can_quarry(cx.w, q) {
                self.dig.begin(cx.w, tier, q, boost(m.id));
                return;
            }
        }
        // No way through: another find.
        self.bad.push(t);
        self.target = None;
        self.last_scan = 0;
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
    fn name(&self) -> &'static str {
        "SeekOre"
    }
}

/// A Moscow doll bound to another doll keeps by it (on patrol or following), teleporting to it from
/// far away; the api crate has her pick up what is dropped around it and empty it when it is full.
#[derive(Clone, Debug)]
struct MoscowFollow {
    recalculate_in: i32,
}

impl MoscowFollow {
    fn doll(m: &Mob, cx: &Cx) -> Option<[f64; 3]> {
        if !(in_mode(m, PATROL) || in_mode(m, FOLLOW)) {
            return None;
        }
        cx.w.seen(bound_to(m.id)?).filter(|s| s.alive).map(pos)
    }
}

impl Goal for MoscowFollow {
    fn flags(&self) -> u8 {
        goals::MOVE | goals::LOOK
    }
    // A dropped item to fetch (round her or the doll she follows) comes first.
    fn can_use(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        item_target(m.id).is_none() && Self::doll(m, cx).is_some_and(|at| distance_sqr(m, at) >= 9.0)
    }
    fn can_continue(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        item_target(m.id).is_none() && Self::doll(m, cx).is_some_and(|at| distance_sqr(m, at) > 2.25)
    }
    fn start(&mut self, _: &mut Mob, _: &mut Cx) {
        self.recalculate_in = 0;
    }
    fn stop(&mut self, m: &mut Mob, _: &mut Cx) {
        m.nav_stop();
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        let Some(at) = Self::doll(m, cx) else { return };
        m.look_at(at);
        self.recalculate_in -= 1;
        if self.recalculate_in > 0 {
            return;
        }
        self.recalculate_in = 10;
        if distance_sqr(m, at) >= TELEPORT_FROM * TELEPORT_FROM {
            teleport_near(m, cx, at);
        } else {
            m.nav_move_to(cx.w, at, 1.0);
        }
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
    fn name(&self) -> &'static str {
        "MoscowFollow"
    }
}

/// A mining doll teleports to a dropped item it has room for (the api crate's item target) that is
/// out of its reach, waits there until it has it, and teleports back to where it was.
#[derive(Clone, Debug, Default)]
struct Fetch {
    back: Option<[f64; 3]>,
    ticks: i32,
    /// Ticks before it tries again, after one it could not get.
    cool: i32,
}

impl Fetch {
    /// One tick: true while it is away fetching, when the caller does nothing else.
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        if self.cool > 0 {
            self.cool -= 1;
        }
        let target = item_target(m.id).filter(|&t| distance_sqr(m, t) <= FETCH_RANGE_SQR);
        let Some(back) = self.back else {
            let Some(t) = target.filter(|&t| distance_sqr(m, t) > 4.0 && self.cool == 0) else { return false };
            self.back = Some(here(m));
            self.ticks = 0;
            Self::go(m, cx, t);
            return true;
        };
        self.ticks += 1;
        match target {
            // Still there: wait beside it (the api crate picks it up within reach).
            Some(t) if self.ticks < FETCH_TICKS => {
                if distance_sqr(m, t) > 4.0 {
                    Self::go(m, cx, t);
                } else {
                    hold_in(m, here(m));
                }
                true
            }
            _ => {
                if target.is_some() {
                    self.cool = 100;
                }
                self.back = None;
                Self::go(m, cx, back);
                false
            }
        }
    }
    fn go(m: &mut Mob, cx: &mut Cx, to: [f64; 3]) {
        hold_in(m, to);
        m.previous = m.position;
        cx.events.push(MobEvent::Sound("entity.enderman.teleport", to, 0.5, 1.4));
    }
}

/// A Horai doll on patrol or following, not quarrying: fetches what it dug and left lying.
#[derive(Clone, Debug, Default)]
struct FetchDrops {
    fetch: Fetch,
}

impl FetchDrops {
    fn allowed(m: &Mob) -> bool {
        (in_mode(m, PATROL) || in_mode(m, FOLLOW)) && !(horai_tier(m) == 2 && in_mode(m, PATROL))
    }
}

impl Goal for FetchDrops {
    fn flags(&self) -> u8 {
        goals::MOVE | goals::LOOK
    }
    fn can_use(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        Self::allowed(m) && self.fetch.tick(m, cx)
    }
    fn can_continue(&mut self, m: &mut Mob, _: &mut Cx) -> bool {
        Self::allowed(m) && self.fetch.back.is_some()
    }
    fn stop(&mut self, m: &mut Mob, cx: &mut Cx) {
        if let Some(back) = self.fetch.back.take() {
            Fetch::go(m, cx, back);
        }
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        self.fetch.tick(m, cx);
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
    fn name(&self) -> &'static str {
        "FetchDrops"
    }
}

/// `TeleportHelper.teleport`, roughly: somewhere within eight blocks, on its feet.
fn dodge(m: &mut Mob) {
    for _ in 0..16 {
        let dx = f64::from(m.random.next_int_bound(17).unwrap_or(8) - 8);
        let dz = f64::from(m.random.next_int_bound(17).unwrap_or(8) - 8);
        if dx.abs() < 2.0 && dz.abs() < 2.0 {
            continue;
        }
        m.position.x += dx;
        m.position.z += dz;
        m.previous = m.position;
        m.nav_stop();
        return;
    }
}

/// The anchor a doll keeps to: on patrol, unless it's the Netherite Horai quarrying.
fn anchor(m: &Mob) -> Option<[f64; 3]> {
    anchor_and_radius(m).map(|(at, _)| at)
}

/// That anchor, and how far a patrolling doll may go from it before it heads back: its radius,
/// 30 to 100 blocks.
fn anchor_and_radius(m: &Mob) -> Option<([f64; 3], f64)> {
    if !in_mode(m, PATROL) || horai_tier(m) == 2 {
        return None;
    }
    let (p, r) = ANCHOR_OF.lock().ok()?.get(&m.id).copied()?;
    Some(([f64::from(p[0]) + 0.5, f64::from(p[1]), f64::from(p[2]) + 0.5], r))
}

/// Past its anchor's radius, a patrolling doll drops what it is doing and heads back; if it hasn't
/// got there in thirty seconds (no path, say), it teleports.
#[derive(Clone, Debug)]
struct AnchorLeash {
    recalculate_in: i32,
    walked: i32,
}

impl Goal for AnchorLeash {
    fn flags(&self) -> u8 {
        goals::MOVE | goals::LOOK
    }
    fn can_use(&mut self, m: &mut Mob, _: &mut Cx) -> bool {
        anchor_and_radius(m).is_some_and(|(at, r)| distance_sqr(m, at) > r * r)
    }
    fn can_continue(&mut self, m: &mut Mob, _: &mut Cx) -> bool {
        anchor(m).is_some_and(|at| distance_sqr(m, at) > 6.0 * 6.0)
    }
    fn start(&mut self, _: &mut Mob, _: &mut Cx) {
        self.recalculate_in = 0;
        self.walked = 0;
    }
    fn stop(&mut self, m: &mut Mob, _: &mut Cx) {
        m.nav_stop();
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        let Some(at) = anchor(m) else { return };
        self.walked += 1;
        if self.walked >= ANCHOR_GIVE_UP {
            self.walked = 0;
            teleport_near(m, cx, at);
            return;
        }
        self.recalculate_in -= 1;
        if self.recalculate_in > 0 {
            return;
        }
        self.recalculate_in = 10;
        // Lost far away (left behind, or stuck), it teleports.
        if distance_sqr(m, at) > 64.0 * 64.0 {
            teleport_near(m, cx, at);
        } else {
            m.nav_move_to(cx.w, at, 1.0);
        }
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
    fn name(&self) -> &'static str {
        "AnchorLeash"
    }
}

/// With nothing to do on patrol, a doll goes home to its anchor (before it would wander); if it
/// hasn't got there in thirty seconds, it teleports.
#[derive(Clone, Debug)]
struct AnchorHome {
    recalculate_in: i32,
    walked: i32,
}

impl Goal for AnchorHome {
    fn flags(&self) -> u8 {
        goals::MOVE | goals::LOOK
    }
    fn can_use(&mut self, m: &mut Mob, _: &mut Cx) -> bool {
        anchor(m).is_some_and(|at| distance_sqr(m, at) > 3.0 * 3.0)
    }
    fn can_continue(&mut self, m: &mut Mob, _: &mut Cx) -> bool {
        anchor(m).is_some_and(|at| distance_sqr(m, at) > 2.0 * 2.0)
    }
    fn start(&mut self, _: &mut Mob, _: &mut Cx) {
        self.recalculate_in = 0;
        self.walked = 0;
    }
    fn stop(&mut self, m: &mut Mob, _: &mut Cx) {
        m.nav_stop();
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        let Some(at) = anchor(m) else { return };
        self.walked += 1;
        if self.walked >= ANCHOR_GIVE_UP {
            self.walked = 0;
            teleport_near(m, cx, at);
            return;
        }
        self.recalculate_in -= 1;
        if self.recalculate_in > 0 {
            return;
        }
        self.recalculate_in = 20;
        m.nav_move_to(cx.w, at, 1.0);
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
    fn name(&self) -> &'static str {
        "AnchorHome"
    }
}

impl Farmer {
    /// Empty farmland within reach gets what she last grew: one a second.
    fn fill_farmland(&self, m: &mut Mob, cx: &mut Cx) {
        let Some(plant) = LAST_CROP.lock().ok().and_then(|l| l.get(&m.id).copied()) else { return };
        let w = cx.w;
        let feet = [m.position.x.floor() as i32, m.position.y.floor() as i32, m.position.z.floor() as i32];
        let r = FARM_RANGE;
        for dy in -2..=1 {
            for dx in -r..=r {
                for dz in -r..=r {
                    let p = add(feet, [dx, dy, dz]);
                    let above = add(p, [0, 1, 0]);
                    if &*name_at(w, p) != "minecraft:farmland" || !is_air(&name_at(w, above)) || stem_beside(w, above) {
                        continue;
                    }
                    if let Some(seed) = CROPS.iter().find(|c| c.0 == plant).map(|c| c.1).or_else(|| STEMS.iter().find(|s| s.0 == plant).map(|s| s.1)) {
                        take_one(m.id, seed);
                    }
                    cx.events.push(MobEvent::SetBlockName { mob: Some(m.id), pos: above, name: plant, effect: false, flags: 3 });
                    cx.events.push(MobEvent::Sound("item.crop.plant", centre(above), 1.0, 1.0));
                    m.look_at(centre(above));
                    m.swing();
                    mark_swing(m.id);
                    return;
                }
            }
        }
    }
}

/// Vanilla's lure time, 5 to 30 seconds, five seconds shorter for each level of Lure on her rod.
fn lure_wait(m: &mut Mob) -> i32 {
    let lure = fishing_enchantments(rod_of(m.id).map(|r| (r.lure, r.luck)), level_of(m.id)).0.min(5) as i32;
    (100 + m.random.next_int_bound(500).unwrap_or(200) - 100 * lure).max(20)
}

/// A catch wears her rod by one, as a player's does (Unbreaking may spare it), and a worn-out rod breaks.
fn wear_rod(m: &mut Mob, rod: Rod) {
    let spared = rod.unbreaking > 0 && m.random.next_int_bound(rod.unbreaking as i32 + 1).unwrap_or(0) != 0;
    if spared {
        return;
    }
    let broke = edit_inventory(m.id, |items| {
        let Some(Some(s)) = items.get_mut(rod.slot) else { return false };
        if &*s.item != "minecraft:fishing_rod" {
            return false;
        }
        s.damage += 1;
        if s.damage >= ROD_DURABILITY {
            items[rod.slot] = None;
            return true;
        }
        false
    })
    .unwrap_or(false);
    if broke {
        say(m.id, 1, "My fishing rod broke!");
    }
}

fn catch(m: &mut Mob, luck: u32) -> &'static str {
    const FISH: [(&str, i32); 4] = [("minecraft:cod", 60), ("minecraft:salmon", 25), ("minecraft:tropical_fish", 2), ("minecraft:pufferfish", 13)];
    const JUNK: [&str; 10] = [
        "minecraft:lily_pad", "minecraft:leather_boots", "minecraft:leather", "minecraft:bone", "minecraft:potion",
        "minecraft:string", "minecraft:bowl", "minecraft:stick", "minecraft:ink_sac", "minecraft:tripwire_hook",
    ];
    const TREASURE: [&str; 5] = ["minecraft:name_tag", "minecraft:saddle", "minecraft:nautilus_shell", "minecraft:bow", "minecraft:fishing_rod"];
    // Luck of the Sea, as vanilla's weights shift: about two points a level from junk to treasure.
    let luck = luck.min(3) as i32;
    let junk = 10 - 2 * luck;
    let treasure = 5 + 2 * luck;
    let roll = m.random.next_int_bound(100).unwrap_or(0);
    if roll < 100 - junk - treasure {
        let mut n = m.random.next_int_bound(100).unwrap_or(0);
        for (fish, w) in FISH {
            if n < w {
                return fish;
            }
            n -= w;
        }
        "minecraft:cod"
    } else if roll < 100 - treasure {
        JUNK[m.random.next_int_bound(JUNK.len() as i32).unwrap_or(0) as usize]
    } else {
        TREASURE[m.random.next_int_bound(TREASURE.len() as i32).unwrap_or(0) as usize]
    }
}

/// Wakasagi, out and about (any mode but standby and riding): with water within four blocks, she
/// casts her rod into it and, every five to thirty seconds as vanilla's lure times go, reels in a
/// catch (`FishingHook`), which goes into her inventory.
#[derive(Clone, Debug, Default)]
struct Fishing {
    spot: Option<[i32; 3]>,
    wait: i32,
    look: i64,
}

impl Fishing {
    fn water_near(m: &Mob, w: &MobWorld) -> Option<[i32; 3]> {
        let feet = [m.position.x.floor() as i32, m.position.y.floor() as i32, m.position.z.floor() as i32];
        let mut best: Option<([i32; 3], i32)> = None;
        for dy in -3..=1 {
            for dx in -4..=4 {
                for dz in -4..=4 {
                    let p = add(feet, [dx, dy, dz]);
                    if &*name_at(w, p) == "minecraft:water" && is_air(&name_at(w, add(p, [0, 1, 0]))) {
                        let d = dx.abs() + dz.abs() + dy.abs();
                        if d >= 2 && best.is_none_or(|b| d < b.1) {
                            best = Some((p, d));
                        }
                    }
                }
            }
        }
        best.map(|b| b.0)
    }
}

impl Goal for Fishing {
    fn flags(&self) -> u8 {
        goals::LOOK
    }
    fn can_use(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        if !(in_mode(m, PATROL) || in_mode(m, FOLLOW)) || cx.w.game_time - self.look < 40 {
            return false;
        }
        self.look = cx.w.game_time;
        self.spot = Self::water_near(m, cx.w);
        self.spot.is_some()
    }
    fn can_continue(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        (in_mode(m, PATROL) || in_mode(m, FOLLOW))
            && self.spot.is_some_and(|p| &*name_at(cx.w, p) == "minecraft:water" && distance_sqr(m, centre(p)) < 49.0)
    }
    fn start(&mut self, m: &mut Mob, cx: &mut Cx) {
        // `FishingHook`'s lure time: 5 to 30 seconds.
        self.wait = quicker(lure_wait(m), boost(m.id));
        if let Some(p) = self.spot {
            cx.events.push(MobEvent::Sound("entity.fishing_bobber.throw", here(m), 0.5, 0.6));
            cx.events.push(MobEvent::ParticleAt { kind: "splash", at: [f64::from(p[0]) + 0.5, f64::from(p[1]) + 1.0, f64::from(p[2]) + 0.5], velocity: [0.0; 3] });
            m.swing();
            mark_swing(m.id);
        }
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        let Some(p) = self.spot else { return };
        let bob = [f64::from(p[0]) + 0.5, f64::from(p[1]) + 0.9, f64::from(p[2]) + 0.5];
        m.look_at(bob);
        self.wait -= 1;
        if self.wait % 40 == 0 {
            cx.events.push(MobEvent::ParticleAt { kind: "fishing", at: bob, velocity: [0.0; 3] });
        }
        if self.wait > 0 {
            return;
        }
        let rod = rod_of(m.id);
        let luck = fishing_enchantments(rod.as_ref().map(|r| (r.lure, r.luck)), level_of(m.id)).1;
        let item = catch(m, luck);
        // Favour: a chance of a second catch, 5% a level up to half.
        let again = (m.random.next_int_bound(100).unwrap_or(99) as u32) < (level_of(m.id) * 5).min(50);
        let second = again.then(|| catch(m, luck));
        if let Some(r) = rod {
            wear_rod(m, r);
        }
        worked(m.id);
        if let Ok(mut c) = CATCHES.lock() {
            c.push((m.id, item));
            if let Some(more) = second {
                c.push((m.id, more));
            }
        }
        cx.events.push(MobEvent::Sound("entity.fishing_bobber.splash", bob, 0.4, 1.0));
        cx.events.push(MobEvent::Sound("entity.fishing_bobber.retrieve", here(m), 0.6, 1.0));
        cx.events.push(MobEvent::ParticleAt { kind: "splash", at: bob, velocity: [0.0, 0.2, 0.0] });
        m.swing();
        mark_swing(m.id);
        self.wait = quicker(lure_wait(m), boost(m.id));
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
    fn name(&self) -> &'static str {
        "Fishing"
    }
}

/// Wakasagi, following her owner, hooks enemies with her line and reels them in to her, as a
/// fishing rod pulls a hooked mob (`FishingHook.pullEntity`: its velocity gains a tenth of the way
/// from it to her). What she hooks is what a following doll fights: whatever is after her owner,
/// and hostile mobs near them. She needs no rod for it, and it wears none.
#[derive(Clone, Debug, Default)]
struct WakasagiHook {
    target: Option<u32>,
    last_scan: i64,
    last_reel: i64,
    flight: i32,
}

impl Goal for WakasagiHook {
    fn flags(&self) -> u8 {
        goals::LOOK
    }
    fn can_use(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        if !in_mode(m, FOLLOW) || cx.w.game_time - self.last_scan < SCAN_EVERY || cx.w.game_time - self.last_reel < HOOK_EVERY {
            return false;
        }
        self.last_scan = cx.w.game_time;
        self.target = nearest(m, cx, |s| {
            (matches!(s.target, Some(Who::Player)) || (enemy(s) && near_owner(m, cx, s))) && !is_doll(s.id)
        })
        .filter(|&id| cx.w.seen(id).is_some_and(|s| distance_sqr(m, pos(s)) <= HOOK_RANGE_SQR) && m.sees(cx.w, Who::Mob(id)));
        self.target.is_some()
    }
    fn can_continue(&mut self, m: &mut Mob, cx: &mut Cx) -> bool {
        in_mode(m, FOLLOW) && self.flight > 0 && self.target.and_then(|id| cx.w.seen(id)).is_some_and(|s| s.alive)
    }
    fn start(&mut self, m: &mut Mob, cx: &mut Cx) {
        self.flight = HOOK_FLIGHT;
        cx.events.push(MobEvent::Sound("entity.fishing_bobber.throw", here(m), 0.5, 0.6));
        m.swing();
        mark_swing(m.id);
    }
    fn stop(&mut self, _: &mut Mob, _: &mut Cx) {
        self.target = None;
        self.flight = 0;
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        let Some(id) = self.target else { return };
        let Some((at, eye)) = cx.w.seen(id).map(|s| (pos(s), s.eye_y)) else { return };
        m.look_at([at[0], eye, at[2]]);
        self.flight -= 1;
        if self.flight > 0 {
            return;
        }
        // Reeled in: pulled a tenth of the way to her, and up a little, so it clears the ground.
        let me = here(m);
        let pull = hook_pull(me, at);
        cx.events.push(MobEvent::Push { target: Who::Mob(id), velocity: pull });
        cx.events.push(MobEvent::Sound("entity.fishing_bobber.retrieve", me, 0.6, 1.0));
        cx.events.push(MobEvent::ParticleAt { kind: "splash", at: [at[0], eye, at[2]], velocity: [0.0, 0.2, 0.0] });
        m.swing();
        mark_swing(m.id);
        worked(m.id);
        self.last_reel = cx.w.game_time;
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
    fn name(&self) -> &'static str {
        "WakasagiHook"
    }
}

/// Aya's photo `m` is taking, from her own eyes: facing `aim` if given, else where her head faces.
fn aya_photo(m: &Mob, aim: Option<[f64; 3]>) -> Photo {
    let me = here(m);
    // Her eyes, a little in front of her face so her own head isn't in the picture.
    let mut eye = [me[0], me[1] + m.kind.eye_height, me[2]];
    let (yaw, pitch) = match aim {
        Some(at) => yaw_pitch(eye, at),
        None => (m.head_yaw, m.head_pitch),
    };
    let (y, p) = (f64::from(yaw).to_radians(), f64::from(pitch).to_radians());
    let ahead = [-y.sin() * p.cos(), -p.sin(), y.cos() * p.cos()];
    for i in 0..3 {
        eye[i] += ahead[i] * 0.3;
    }
    Photo { doll: m.id, eye, yaw, pitch }
}

/// Punched with an empty hand, in any mode, Ooedo hisses as a creeper does and explodes with a
/// creeper's blast (which, as every blast, it lives through).
#[derive(Clone, Debug, Default)]
struct Detonate {
    fuse: i32,
}

impl Goal for Detonate {
    fn flags(&self) -> u8 {
        goals::MOVE | goals::LOOK
    }
    fn can_use(&mut self, m: &mut Mob, _: &mut Cx) -> bool {
        DETONATE.lock().is_ok_and(|mut d| d.remove(&m.id))
    }
    fn can_continue(&mut self, _: &mut Mob, _: &mut Cx) -> bool {
        self.fuse < PUNCH_FUSE
    }
    fn start(&mut self, m: &mut Mob, cx: &mut Cx) {
        self.fuse = 0;
        m.nav_stop();
        cx.events.push(MobEvent::Sound("entity.creeper.primed", here(m), 1.0, 0.5));
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        self.fuse += 1;
        if self.fuse % 5 == 0 {
            let me = here(m);
            cx.events.push(MobEvent::ParticleAt { kind: "smoke", at: [me[0], me[1] + 0.6, me[2]], velocity: [0.0, 0.05, 0.0] });
        }
        if self.fuse >= PUNCH_FUSE {
            cx.events.push(MobEvent::Explode(here(m), blast(m.id, EXPLODE_STRENGTH), m.species(), false));
            blasted(m.id);
        }
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
    fn name(&self) -> &'static str {
        "OoedoPunched"
    }
}

#[derive(Clone, Debug)]
struct Mission;

impl Goal for Mission {
    fn flags(&self) -> u8 {
        goals::MOVE | goals::LOOK | goals::JUMP
    }
    fn can_use(&mut self, m: &mut Mob, _: &mut Cx) -> bool {
        on_mission(m.id)
    }
    fn every_tick(&self) -> bool {
        true
    }
    fn tick(&mut self, m: &mut Mob, cx: &mut Cx) {
        m.nav_stop();
        m.fall_distance = 0.0;
        if BURST.lock().is_ok_and(|mut b| b.remove(&m.id)) {
            cx.events.push(MobEvent::Explode(here(m), blast(m.id, GUIGNOL_BLAST), m.species(), false));
        }
    }
    fn box_clone(&self) -> Box<dyn Goal> {
        Box::new(self.clone())
    }
    fn name(&self) -> &'static str {
        "GuignolMission"
    }
}
