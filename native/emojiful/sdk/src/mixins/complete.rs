// SPDX-License-Identifier: MIT
fn complete(word: &str) -> core::ops::ControlFlow<Vec<String>> {
    let Some(prefix) = word.strip_prefix(':') else {
        return core::ops::ControlFlow::Continue(());
    };
    if prefix.chars().count() < 2 || prefix.ends_with(':') {
        return core::ops::ControlFlow::Continue(());
    }
    let prefix = prefix.to_ascii_lowercase();
    let names = crate::chat::emojiful_names::EMOJI;
    let start = names.partition_point(|(name, _)| *name < prefix.as_str());
    let matches = names[start..]
        .iter()
        .take_while(|(name, _)| name.starts_with(&prefix))
        .map(|(name, _)| format!(":{name}:"))
        .collect();
    core::ops::ControlFlow::Break(matches)
}
