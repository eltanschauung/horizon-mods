// Shared threshold policy and a bounded-cost, per-game-home settings cache.
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
pub const ID: &str = "sleep_threshold";
pub const LABELS: [&str; 7] = [
    "One Player",
    ">=10%",
    ">=25%",
    ">=33%",
    ">=50%",
    ">=75%",
    "Vanilla",
];
pub fn label(choice: usize) -> String {
    format!("Sleeping required: {}", LABELS[choice])
}
type Cache = HashMap<PathBuf, (Instant, usize)>;
static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
fn cache() -> &'static Mutex<Cache> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}
pub fn choice(path: &Path) -> usize {
    let mut cache = cache().lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, choice)) = cache.get(path) {
        if at.elapsed() < Duration::from_secs(1) {
            return *choice;
        }
    }
    let text = std::fs::read_to_string(path.join("threshold")).unwrap_or_default();
    let choice = LABELS
        .iter()
        .position(|l| l.eq_ignore_ascii_case(text.trim()))
        .unwrap_or(0);
    cache.insert(path.to_owned(), (Instant::now(), choice));
    choice
}
pub fn invalidate(path: &Path) {
    cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(path);
}
/// None delegates to vanilla's gamerule. At least one eligible player must sleep.
pub fn needed(choice: usize, active: i64) -> Option<i64> {
    let percent = match choice {
        0 => return Some(1),
        1 => 10,
        2 => 25,
        3 => 33,
        4 => 50,
        5 => 75,
        _ => return None,
    };
    let active = active.max(0);
    Some((active / 100 * percent + (active % 100 * percent + 99) / 100).max(1))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn menu_changes_reload_and_game_homes_stay_separate() {
        let root = std::env::temp_dir().join(format!("sleep-threshold-{}", std::process::id()));
        let a = root.join("a");
        let b = root.join("b");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        assert_eq!(choice(&a), 0);
        std::fs::write(a.join("threshold"), ">=50%").unwrap();
        invalidate(&a);
        assert_eq!(choice(&a), 4);
        assert_eq!(choice(&b), 0);
        std::fs::write(a.join("threshold"), "Vanilla").unwrap();
        invalidate(&a);
        assert_eq!(choice(&a), 6);
        std::fs::remove_dir_all(root).unwrap();
        invalidate(&a);
        invalidate(&b);
    }
    #[test]
    fn fractional_thresholds_require_the_next_whole_player() {
        for active in 1..=128 {
            assert_eq!(needed(0, active), Some(1));
            for (choice, percent) in [(1, 10), (2, 25), (3, 33), (4, 50), (5, 75)] {
                let n = needed(choice, active).unwrap();
                assert!(n * 100 >= active * percent);
                assert!((n - 1) * 100 < active * percent);
            }
        }
        assert_eq!(needed(4, 5), Some(3));
        assert_eq!(needed(5, 5), Some(4));
        assert_eq!(needed(3, 3), Some(1));
        assert_eq!(needed(6, 5), None);
        assert_eq!(needed(1, 0), Some(1));
        assert!(needed(5, i64::MAX).unwrap() > 0);
    }
}
