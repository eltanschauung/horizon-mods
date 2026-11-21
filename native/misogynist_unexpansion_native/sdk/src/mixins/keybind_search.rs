// Key search inspired by Controlling by Jaredlll08 (MIT).
fn match_bound_key(key: &str, query: &str) -> std::ops::ControlFlow<bool> {
    if query.is_empty() {
        return std::ops::ControlFlow::Continue(());
    }
    let code = key.trim_start_matches("key.keyboard.").trim_start_matches("key.").replace('.', " ");
    if code.to_lowercase().contains(query) || crate::lang::get(key).to_lowercase().contains(query) {
        std::ops::ControlFlow::Break(true)
    } else {
        std::ops::ControlFlow::Continue(())
    }
}
