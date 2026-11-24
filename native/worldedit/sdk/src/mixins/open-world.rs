// Entering a world makes its schematics folder, so the menu never lists a
// folder that is not there, and notes the world for the chat completions.
fn open_world_head(game: &crate::game::Game<'_>, folder: &str) {
    ::worldedit::paths::ensure_world(&game.instance_dir().join("saves").join(folder));
}
