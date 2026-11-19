// SPDX-License-Identifier: GPL-3.0-or-later
use super::Game;
use std::{
    cell::RefCell,
    io::{BufRead, BufReader, Read},
    time::{Duration, Instant},
};

const CHANNEL: &str = "corpse_native:waypoints";
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
    let path = root.join("corpse").join("corpses.txt");
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let owner = format!("{:032x}", game.command_actor_uuid());
    BufReader::new(file.take(1_048_576))
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| {
            let mut fields = line.split('\t');
            if fields.next()? != "corpse" {
                return None;
            }
            fields.next()?; // corpse id
            if fields.next()? != owner {
                return None;
            }
            fields.next()?; // owner name
            if fields.next()? != dimension {
                return None;
            }
            Some([
                fields.next()?.parse().ok()?,
                fields.next()?.parse().ok()?,
                fields.next()?.parse().ok()?,
            ])
        })
        .filter(valid)
        .take(MAX)
        .collect()
}

pub(super) fn points(game: &Game<'_>) -> Vec<[f64; 3]> {
    let scope = super::minimap_waypoints::scope(game);
    let dimension = game.dim.id();
    SNAPSHOT.with_borrow_mut(|s| {
        let current = s.scope == scope && s.dimension == dimension;
        if game.remote.is_none()
            && game.native_on("corpse_native")
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
