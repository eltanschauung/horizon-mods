// SPDX-License-Identifier: MIT

/// Skip the night once 33% of eligible players, rounded up, are asleep.
pub fn players_needed(active: i64) -> i64 {
    let active = active.max(0);
    // Divide before multiplying so even a hypothetical i64::MAX player count
    // rounds correctly without overflow.
    ((active / 100) * 33 + ((active % 100) * 33 + 99) / 100).max(1)
}

#[cfg(test)]
mod tests {
    use super::players_needed;

    #[test]
    fn requires_at_least_one_and_rounds_up() {
        assert_eq!([-1, 0, 1, 2, 3, 4, 6, 7, 100, 101].map(players_needed),
                   [1, 1, 1, 1, 1, 2, 2, 3, 33, 34]);
    }
}
