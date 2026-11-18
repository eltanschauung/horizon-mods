// Port of Gensokyo Legacy's placement filters by lcy0x1 / Signifier-Dollhouse.
// LGPL-2.1; see LICENSE and THIRD_PARTY_NOTICES.md.
use super::*;
use crate::heightmap::HeightmapType;
use horizon_core::random::{PositionalRandom, XoroshiroRandom};
use serde_json::Value;
#[derive(Clone, Debug)]
pub struct Modifier(Value);
impl Modifier {
    pub fn decode(v: Value) -> Result<Self, DecodeError> {
        match v["type"].as_str().unwrap_or("") {
            "horizon_gensokyo_legacy:biome_density"
                if ["edge", "interior"]
                    .iter()
                    .all(|key| v[key].as_f64().is_some_and(|n| (0.0..=1.0).contains(&n)))
                    && v["salt"].as_i64().is_some() =>
            {
                Ok(Self(v))
            }
            "horizon_gensokyo_legacy:jittered_grid"
            | "horizon_gensokyo_legacy:grid_exclusion"
            | "horizon_gensokyo_legacy:noise_band"
            | "horizon_gensokyo_legacy:canopy"
            | "horizon_gensokyo_legacy:avoid_structures" => Ok(Self(v)),
            _ => Err(DecodeError::Field("unknown Gensokyo placement")),
        }
    }
    pub fn positions(
        &self,
        c: &mut dyn PlacementContext,
        p: [i32; 3],
    ) -> Result<Vec<[i32; 3]>, PlacementQueryError> {
        let v = &self.0;
        let k = v["type"].as_str().unwrap();
        let keep = match k {
            // The referenced houses are deliberately outside this biome-only port; no starts of
            // these IDs exist. Keep this filter explicit so adding structures cannot hide it.
            "horizon_gensokyo_legacy:avoid_structures" => true,
            "horizon_gensokyo_legacy:canopy" => {
                let top = c.height(HeightmapType::WorldSurface, p[0], p[2])?;
                let floor = c.height(HeightmapType::MotionBlockingNoLeaves, p[0], p[2])?;
                (top - floor >= int(v, "min_gap")) == v["under"].as_bool().unwrap()
            }
            "horizon_gensokyo_legacy:biome_density" => {
                // Measure distance to the nearest biome edge in rings, without querying
                // beyond the FEATURES stage's 3×3 terrain window.
                let [x, y, z] = p;
                let y = y + 4;
                let biome = c.biome_identity([x, y, z])?;
                let mut interior = 1.0;
                'rings: for radius in [4, 9, 14] {
                    let diagonal = radius * 7 / 10;
                    for (dx, dz) in [
                        (radius, 0),
                        (-radius, 0),
                        (0, radius),
                        (0, -radius),
                        (diagonal, diagonal),
                        (diagonal, -diagonal),
                        (-diagonal, diagonal),
                        (-diagonal, -diagonal),
                    ] {
                        if c.biome_identity([x.wrapping_add(dx), y, z.wrapping_add(dz)])? != biome {
                            interior = f64::from(radius - 4) / 15.0;
                            break 'rings;
                        }
                    }
                }
                let edge = v["edge"].as_f64().unwrap();
                let chance = edge + interior * (v["interior"].as_f64().unwrap() - edge);
                let seed = c.world_seed()? as u64 ^ v["salt"].as_i64().unwrap() as u64;
                let at = ((x as u32 as u64) << 32) | (z as u32 as u64);
                ((mix(seed ^ mix(at)) >> 11) as f64) < chance * (1u64 << 53) as f64
            }
            "horizon_gensokyo_legacy:jittered_grid" => {
                let layer = &v["value"];
                let cell = int(layer, "cell");
                let seed = c.world_seed()?;
                let mut out = Vec::new();
                let x = p[0].div_euclid(16) * 16;
                let z = p[2].div_euclid(16) * 16;
                for cx in x.div_euclid(cell)..=(x + 15).div_euclid(cell) {
                    for cz in z.div_euclid(cell)..=(z + 15).div_euclid(cell) {
                        if let Some([px, _, pz]) = point(layer, seed, cx, cz) {
                            if px >= x && px < x + 16 && pz >= z && pz < z + 16 {
                                out.push([px, p[1], pz]);
                            }
                        }
                    }
                }
                return Ok(out);
            }
            "horizon_gensokyo_legacy:grid_exclusion" => {
                let seed = c.world_seed()?;
                let mut near = false;
                for e in v["layers"].as_array().unwrap() {
                    let l = &e["layer"];
                    let cell = int(l, "cell");
                    let d = int(e, "distance");
                    for x in (p[0] - d).div_euclid(cell)..=(p[0] + d).div_euclid(cell) {
                        for z in (p[2] - d).div_euclid(cell)..=(p[2] + d).div_euclid(cell) {
                            if let Some(q) = point(l, seed, x, z) {
                                let dx = i64::from(q[0] - p[0]);
                                let dz = i64::from(q[2] - p[2]);
                                near |= dx * dx + dz * dz < i64::from(d * d);
                            }
                        }
                    }
                }
                !near
            }
            "horizon_gensokyo_legacy:noise_band" => {
                use horizon_core::noise::Noise;
                use std::sync::{Arc, Mutex, OnceLock};
                type Entry = (i64, String, Arc<dyn Noise>);
                static CACHE: OnceLock<Mutex<Vec<Entry>>> = OnceLock::new();
                let seed = c.world_seed()?;
                let name = v["noise"].as_str().unwrap();
                let mut cache = CACHE
                    .get_or_init(Mutex::default)
                    .lock()
                    .map_err(|_| PlacementQueryError::Failed("Gensokyo noise cache".into()))?;
                if !cache.iter().any(|e| e.0 == seed && e.1 == name) {
                    let text = match name {
                        "horizon_gensokyo_legacy:magical_forest_type" => {
                            r#"{"firstOctave":-7,"amplitudes":[1.0,0.5]}"#
                        }
                        "horizon_gensokyo_legacy:magical_forest_clearing" => {
                            r#"{"firstOctave":-5,"amplitudes":[1.0]}"#
                        }
                        _ => {
                            return Err(PlacementQueryError::Failed(format!(
                                "unknown noise {name}"
                            )));
                        }
                    };
                    let factory = XoroshiroRandom::new(seed).fork_positional();
                    let noise = crate::worldgen::decode_noise(text)
                        .map_err(|e| PlacementQueryError::Failed(e.to_string()))?
                        .create(&mut factory.from_hash_of(name))
                        .map_err(|e| PlacementQueryError::Failed(e.to_string()))?;
                    if cache.len() >= 8 {
                        cache.remove(0);
                    }
                    cache.push((seed, name.into(), Arc::new(noise)));
                }
                let n = cache
                    .iter()
                    .find(|e| e.0 == seed && e.1 == name)
                    .unwrap()
                    .2
                    .get3(p[0] as f64, 0., p[2] as f64) as f64;
                n >= v["min"].as_f64().unwrap_or(f64::NEG_INFINITY)
                    && n < v["max"].as_f64().unwrap_or(f64::INFINITY)
            }
            _ => unreachable!(),
        };
        Ok(if keep { vec![p] } else { vec![] })
    }
}
fn int(v: &Value, k: &str) -> i32 {
    v[k].as_i64().unwrap() as i32
}
fn mix(mut n: u64) -> u64 {
    n = (n ^ (n >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    n = (n ^ (n >> 27)).wrapping_mul(0x94d049bb133111eb);
    n ^ (n >> 31)
}
fn point(v: &Value, seed: i64, x: i32, z: i32) -> Option<[i32; 3]> {
    let lo = mix((seed as u64) ^ ((int(v, "salt") as i64 as u64).wrapping_mul(0x9e3779b97f4a7c15)));
    let hi = mix((x as i64 as u64)
        .wrapping_mul(0x632be59bd9b4e019)
        .wrapping_add((z as i64 as u64).wrapping_mul(0x2545f4914f6cdd1d)));
    let mut r = XoroshiroRandom::from_state(lo ^ hi, hi);
    let u = r.next_float();
    let cell = int(v, "cell");
    let m = int(v, "margin");
    let px = x * cell + m + r.next_int_bound(cell - 2 * m).unwrap();
    let pz = z * cell + m + r.next_int_bound(cell - 2 * m).unwrap();
    (u < (v["chance"].as_f64().unwrap() as f32)).then_some([px, 0, pz])
}

impl NativePlacement for Modifier {
    fn positions(
        &self,
        context: &mut dyn PlacementContext,
        p: [i32; 3],
    ) -> Result<Vec<[i32; 3]>, PlacementQueryError> {
        self.positions(context, p)
    }
}
