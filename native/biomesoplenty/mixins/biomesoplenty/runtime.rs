// Native translations from the authorized BOP 26.3.0.0.7 reference.
// Copyright Glitchfiend Team; original license and user authorization retained in source.json.
use super::{NativeAlgorithm, NativeFeature, NativeRandom, NativeWorld};
use crate::registry::BlockStateId as State;
use crate::worldgen::{
    DecodeError, block_feature::FeatureBlockBindings, placement::PlacementQueryError as Error,
};
use std::{collections::BTreeSet, sync::Arc};
type Result<T> = std::result::Result<T, Error>;
#[derive(Debug)]
pub struct HighGrass {
    air: BTreeSet<State>,
    grass: BTreeSet<State>,
    heads: Vec<State>,
    body: State,
}
#[derive(Debug)]
pub struct SmallFumarole {
    replace: BTreeSet<State>,
    buds: BTreeSet<State>,
    ground: BTreeSet<State>,
    solid: State,
    vent: State,
    air: State,
}
fn states(b: &FeatureBlockBindings<'_>, names: &[&str]) -> BTreeSet<State> {
    let r = b.registry();
    (0..r.len())
        .filter_map(|i| {
            let id = State(i as u32);
            names.contains(&r.state(id)?.name.as_str()).then_some(id)
        })
        .collect()
}
pub fn decode(
    kind: &str,
    b: &FeatureBlockBindings<'_>,
) -> Option<std::result::Result<NativeFeature, DecodeError>> {
    if !matches!(
        kind,
        "biomesoplenty:high_grass" | "biomesoplenty:small_fumarole"
    ) {
        return None;
    }
    Some((|| {
        let r = b.registry();
        let get = |name: &str| {
            r.default_state(name)
                .ok_or_else(|| DecodeError::Registry(format!("BOP feature requires {name}")))
        };
        let air = states(
            b,
            &["minecraft:air", "minecraft:cave_air", "minecraft:void_air"],
        );
        let algorithm: Arc<dyn NativeAlgorithm> = if kind == "biomesoplenty:high_grass" {
            let head = get("biomesoplenty:high_grass")?;
            let mut heads = Vec::new();
            for age in 0..=25 {
                let mut state = r.state(head).unwrap().clone();
                state.properties.insert("age".into(), age.to_string());
                heads.push(
                    r.id(&state)
                        .ok_or(DecodeError::Field("BOP high grass age domain"))?,
                );
            }
            Arc::new(HighGrass {
                air,
                grass: states(b, &["minecraft:grass_block"]),
                heads,
                body: get("biomesoplenty:high_grass_plant")?,
            })
        } else {
            let mut replace = air;
            if let Some(leaves) = b.tags.get("minecraft:leaves") {
                replace.extend(leaves.iter().copied());
            }
            Arc::new(SmallFumarole {
                replace,
                buds: states(
                    b,
                    &[
                        "biomesoplenty:orpiment_bud",
                        "biomesoplenty:orpiment_cluster",
                    ],
                ),
                ground: states(b, &["biomesoplenty:orpiment"]),
                solid: get("biomesoplenty:orpiment")?,
                vent: get("biomesoplenty:orpiment_fumarole")?,
                air: get("minecraft:air")?,
            })
        };
        Ok(NativeFeature(algorithm))
    })())
}
impl HighGrass {
    fn valid(&self, w: &mut dyn NativeWorld, p: [i32; 3]) -> Result<bool> {
        Ok(self.air.contains(&w.block(p)?)
            && self.grass.contains(&w.block([p[0], p[1] - 1, p[2]])?))
    }
}
impl NativeAlgorithm for HighGrass {
    fn place(
        &self,
        w: &mut dyn NativeWorld,
        r: &mut dyn NativeRandom,
        p: [i32; 3],
    ) -> Result<bool> {
        if !self.valid(w, p)? {
            return Ok(false);
        }
        let min = w.min_y()?;
        let max = w.max_y()?;
        for _ in 0..25 {
            // BOP consumes X, Z, then Y, in this order.
            let x = r.int(11)? - 5;
            let z = r.int(11)? - 5;
            let mut q = [p[0] + x, p[1] + r.int(7)? - 3, p[2] + z];
            loop {
                q[1] -= 1;
                if q[1] < min || q[1] > max {
                    break;
                }
                if !self.air.contains(&w.block(q)?) {
                    q[1] += 1;
                    break;
                }
            }
            if q[1] < min || q[1] > max || !self.valid(w, q)? {
                continue;
            }
            let height = 1 + ((10 - (x.abs() + z.abs())) / 2);
            for i in 1..=height {
                if self.air.contains(&w.block(q)?) {
                    if i == height || !self.air.contains(&w.block([q[0], q[1] + 1, q[2]])?) {
                        w.set(q, self.heads[(7 + r.int(4)?) as usize], 2)?;
                        break;
                    }
                    w.set(q, self.body, 2)?;
                }
                q[1] += 1;
            }
        }
        Ok(true)
    }
}
impl NativeAlgorithm for SmallFumarole {
    fn place(
        &self,
        w: &mut dyn NativeWorld,
        r: &mut dyn NativeRandom,
        p: [i32; 3],
    ) -> Result<bool> {
        let mut count = 0;
        for _ in 0..64 {
            let q = [
                p[0] + r.int(8)? - r.int(8)?,
                p[1] + r.int(4)? - r.int(4)?,
                p[2] + r.int(8)? - r.int(8)?,
            ];
            // The reference deliberately checks buds at the origin, not the candidate.
            if (self.replace.contains(&w.block(q)?) || self.buds.contains(&w.block(p)?))
                && self.ground.contains(&w.block([q[0], q[1] - 1, q[2]])?)
            {
                if r.int(5)? == 0 {
                    let upper = [q[0], q[1] + 1, q[2]];
                    if r.int(2)? == 0 {
                        w.set(q, self.solid, 2)?;
                        w.set(upper, self.vent, 2)?;
                    } else {
                        w.set(q, self.vent, 2)?;
                        w.set(upper, self.air, 2)?;
                    }
                }
                count += 1;
            }
        }
        Ok(count > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    struct Random {
        calls: usize,
    }
    impl NativeRandom for Random {
        fn int(&mut self, bound: i32) -> Result<i32> {
            self.calls += 1;
            Ok(if bound == 7 { 3 } else { 0 })
        }
        fn float(&mut self) -> f32 {
            0.0
        }
        fn double(&mut self) -> f64 {
            0.0
        }
        fn boolean(&mut self) -> bool {
            false
        }
    }
    struct World {
        ground: State,
        cells: BTreeMap<[i32; 3], State>,
        writes: Vec<([i32; 3], State, i32)>,
    }
    impl NativeWorld for World {
        fn block(&mut self, p: [i32; 3]) -> Result<State> {
            Ok(*self
                .cells
                .get(&p)
                .unwrap_or(if p[1] <= 60 { &self.ground } else { &State(0) }))
        }
        fn set(&mut self, p: [i32; 3], s: State, f: i32) -> Result<bool> {
            self.cells.insert(p, s);
            self.writes.push((p, s, f));
            Ok(true)
        }
        fn min_y(&mut self) -> Result<i32> {
            Ok(-64)
        }
        fn max_y(&mut self) -> Result<i32> {
            Ok(319)
        }
    }
    fn world(ground: u32) -> World {
        World {
            ground: State(ground),
            cells: BTreeMap::new(),
            writes: Vec::new(),
        }
    }
    #[test]
    fn high_grass_reference_draw_order_and_height() {
        let feature = HighGrass {
            air: [State(0)].into(),
            grass: [State(1)].into(),
            heads: (0..26).map(|a| State(10 + a)).collect(),
            body: State(2),
        };
        let mut w = world(1);
        let mut r = Random { calls: 0 };
        assert!(feature.place(&mut w, &mut r, [0, 61, 0]).unwrap());
        assert_eq!(w.writes, vec![([-5, 61, -5], State(17), 2)]);
        assert_eq!(r.calls, 76);
        let mut water = world(3);
        let mut r = Random { calls: 0 };
        assert!(!feature.place(&mut water, &mut r, [0, 61, 0]).unwrap());
        assert!(water.writes.is_empty());
        assert_eq!(r.calls, 0);
    }
    #[test]
    fn fumarole_reference_random_consumption_and_ordered_writes() {
        let feature = SmallFumarole {
            replace: [State(0)].into(),
            buds: [State(5), State(6)].into(),
            ground: [State(1)].into(),
            solid: State(1),
            vent: State(2),
            air: State(0),
        };
        let mut w = world(1);
        let mut r = Random { calls: 0 };
        assert!(feature.place(&mut w, &mut r, [0, 61, 0]).unwrap());
        assert_eq!(
            w.writes,
            vec![([0, 61, 0], State(1), 2), ([0, 62, 0], State(2), 2)]
        );
        assert_eq!(r.calls, 386);
        let mut water = world(3);
        let mut r = Random { calls: 0 };
        assert!(!feature.place(&mut water, &mut r, [0, 61, 0]).unwrap());
        assert!(water.writes.is_empty());
        assert_eq!(r.calls, 384);
    }
}
