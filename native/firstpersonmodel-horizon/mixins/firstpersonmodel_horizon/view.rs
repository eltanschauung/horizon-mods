// SPDX-License-Identifier: MIT
// A render-scoped override shared with the existing player and armor poser.
use std::cell::Cell;
thread_local! { static HIDE_ARMS: Cell<bool> = const { Cell::new(false) }; }
pub fn arms_hidden() -> bool { HIDE_ARMS.with(Cell::get) }
pub fn with_hidden_arms<T>(hidden: bool, draw: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore { fn drop(&mut self) { HIDE_ARMS.with(|state| state.set(self.0)); } }
    let _restore = Restore(HIDE_ARMS.with(|state| state.replace(hidden)));
    draw()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn nested_scopes_restore_the_previous_view() {
        assert!(!arms_hidden());
        with_hidden_arms(true, || {
            assert!(arms_hidden());
            with_hidden_arms(false, || assert!(!arms_hidden()));
            assert!(arms_hidden());
        });
        assert!(!arms_hidden());
    }
    #[test] fn unwinding_restores_the_view() {
        let _ = std::panic::catch_unwind(|| with_hidden_arms(true, || panic!("fixture")));
        assert!(!arms_hidden());
    }
}
