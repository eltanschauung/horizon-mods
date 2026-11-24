//! A coarse index over a tick's snapshot. Queries keep snapshot order for first-hit collisions.
use std::collections::HashMap;

type Cell = (i32, i32, i32);
pub type Bounds = [[f64; 3]; 2];
const CELL_SIZE: f64 = 8.0;
const MAX_CELLS: i64 = 512;

#[derive(Default)]
pub struct Grid {
    cells: HashMap<Cell, Vec<usize>>,
    wide: Vec<usize>,
    len: usize,
}

fn cells(bounds: Bounds) -> Option<impl Iterator<Item = Cell>> {
    if bounds.iter().flatten().any(|v| !v.is_finite()) {
        return None;
    }
    let [min, max] = bounds.map(|point| point.map(|v| (v / CELL_SIZE).floor() as i32));
    let mut volume = 1;
    for (lo, hi) in min.into_iter().zip(max) {
        let count = i64::from(hi) - i64::from(lo) + 1;
        if count <= 0 || count > MAX_CELLS / volume {
            return None;
        }
        volume *= count;
    }
    Some((min[0]..=max[0]).flat_map(move |x| {
        (min[1]..=max[1]).flat_map(move |y| (min[2]..=max[2]).map(move |z| (x, y, z)))
    }))
}

impl Grid {
    pub fn insert(&mut self, bounds: Bounds) {
        let index = self.len;
        self.len += 1;
        if let Some(cells) = cells(bounds) {
            for cell in cells {
                self.cells.entry(cell).or_default().push(index);
            }
        } else {
            self.wide.push(index);
        }
    }

    pub fn query<'a>(&self, bounds: Bounds, hits: &'a mut Vec<usize>) -> &'a [usize] {
        hits.clear();
        if let Some(cells) = cells(bounds) {
            hits.extend_from_slice(&self.wide);
            for cell in cells {
                if let Some(entries) = self.cells.get(&cell) {
                    hits.extend_from_slice(entries);
                }
            }
            hits.sort_unstable();
            hits.dedup();
        } else {
            hits.extend(0..self.len);
        }
        hits
    }
}

pub fn swept(from: [f64; 3], to: [f64; 3], padding: f64) -> Bounds {
    [
        std::array::from_fn(|i| from[i].min(to[i]) - padding),
        std::array::from_fn(|i| from[i].max(to[i]) + padding),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_candidates_in_snapshot_order_without_duplicates() {
        let mut grid = Grid::default();
        grid.insert([[8.0; 3], [16.0; 3]]);
        grid.insert([[-8.0; 3], [8.0; 3]]);
        grid.insert([[999.0; 3], [1000.0; 3]]);
        grid.insert([[-1000.0; 3], [1000.0; 3]]);
        let mut hits = Vec::new();
        assert_eq!(grid.query([[-1.0; 3], [8.0; 3]], &mut hits), &[0, 1, 3]);
        assert_eq!(grid.query([[-999.0; 3], [999.0; 3]], &mut hits), &[0, 1, 2, 3]);
    }
}
