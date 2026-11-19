// SPDX-License-Identifier: GPL-3.0-or-later
use super::Game;
use std::{
    cell::RefCell,
    io::Read,
    time::{Duration, Instant},
};

const CHANNEL: &str = "gravestone_core:waypoints";
const MAX: usize = 64;

#[derive(Default)]
struct Snapshot {
    scope: String,
    dimension: String,
    received: Option<Instant>,
    points: Vec<[f64; 3]>,
}
thread_local! { static SNAPSHOT: RefCell<Snapshot> = RefCell::new(Snapshot::default()); }

fn valid(p: &[f64; 3]) -> bool {
    p.iter().all(|v| v.is_finite() && v.abs() <= 30_000_000.0)
}

fn decode(bytes: &[u8]) -> Option<(&str, Vec<[f64; 3]>)> {
    let [1, dimension_len, tail @ ..] = bytes else {
        return None;
    };
    let (dimension, positions) = tail.split_at_checked(usize::from(*dimension_len))?;
    if !positions.len().is_multiple_of(24) || positions.len() > MAX * 24 {
        return None;
    }
    let dimension = std::str::from_utf8(dimension).ok()?;
    let points: Vec<_> = positions
        .as_chunks::<24>()
        .0
        .iter()
        .map(|point| {
            std::array::from_fn(|i| {
                f64::from_be_bytes(point[i * 8..(i + 1) * 8].try_into().expect("eight bytes"))
            })
        })
        .filter(valid)
        .collect();
    Some((dimension, points))
}

pub(super) fn receive(game: &Game<'_>, channel: &str, bytes: &[u8]) {
    if channel != CHANNEL || game.remote.is_none() || !game.native_on("minimap") {
        return;
    }
    let Some((dimension, points)) = decode(bytes) else {
        return;
    };
    SNAPSHOT.with_borrow_mut(|s| {
        *s = Snapshot {
            scope: super::minimap_waypoints::scope(game),
            dimension: dimension.into(),
            received: Some(Instant::now()),
            points,
        };
    });
}

fn read_local(game: &Game<'_>, dimension: &str) -> Vec<[f64; 3]> {
    let Some(root) = game.native_world_dir() else {
        return Vec::new();
    };
    let path = root.join("gravestone").join("graves.json");
    let Ok(file) = std::fs::File::open(path) else { return Vec::new(); };
    let mut bytes = Vec::new();
    if file.take(16 * 1024 * 1024 + 1).read_to_end(&mut bytes).is_err() {
        return Vec::new();
    }
    #[derive(serde::Deserialize)]
    struct Grave { player_uuid: u128, dimension: String, pos: [i32; 3] }
    let Ok(graves) = serde_json::from_slice::<Vec<Grave>>(&bytes) else { return Vec::new(); };
    let owner = game.command_actor_uuid();
    graves.into_iter().filter(|g| g.player_uuid == owner && g.dimension == dimension)
        .map(|g| { let [x, y, z] = g.pos.map(f64::from); [x + 0.5, y, z + 0.5] })
        .filter(valid).take(MAX).collect()
}

pub(super) fn points(game: &Game<'_>) -> Vec<[f64; 3]> {
    let scope = super::minimap_waypoints::scope(game);
    let dimension = game.dim.id();
    SNAPSHOT.with_borrow_mut(|s| {
        let current = s.scope == scope && s.dimension == dimension;
        if game.remote.is_none()
            && game.native_on("gravestone_core")
            && (!current
                || s.received
                    .is_none_or(|at| at.elapsed() >= Duration::from_secs(1)))
        {
            *s = Snapshot {
                points: read_local(game, dimension),
                dimension: dimension.into(),
                scope,
                received: Some(Instant::now()),
            };
            return s.points.clone();
        }
        if current
            && s.received
                .is_some_and(|at| at.elapsed() < Duration::from_secs(3))
        {
            s.points.clone()
        } else {
            Vec::new()
        }
    })
}
