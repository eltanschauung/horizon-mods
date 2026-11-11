// SPDX-License-Identifier: MIT
use std::cell::Cell;
thread_local! { static HIDE_EMPTY: Cell<bool> = const { Cell::new(false) }; }
pub fn hide_empty() -> bool { HIDE_EMPTY.with(Cell::get) }
pub fn with_empty_hidden<T>(hidden: bool, render: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore { fn drop(&mut self) { HIDE_EMPTY.with(|v| v.set(self.0)); } }
    let _restore = Restore(HIDE_EMPTY.with(|v| v.replace(hidden)));
    render()
}
