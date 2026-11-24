//! The chat's completions for the mod's commands: schematic files for `/hm load`, placements for
//! `/hm select` and `/hm unload`. Command argument types of the mod's own
//! (`horizonmatica:schematic`, `horizonmatica:placement`); the game asks them as the line is
//! typed, without a host, so what they offer is kept here as it changes.

use mod_api::{CommandArgumentError, CommandArgumentKind, CommandArgumentType, CommandSender, CommandSuggestion, Registrar};
use std::path::PathBuf;
use std::sync::Mutex;

/// The schematics folder, once known.
static FOLDER: Mutex<Option<PathBuf>> = Mutex::new(None);
/// The loaded placements' names.
static PLACEMENTS: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub fn set_folder(dir: PathBuf) {
    *FOLDER.lock().unwrap_or_else(|e| e.into_inner()) = Some(dir);
}

pub fn set_placements(names: Vec<String>) {
    let mut p = PLACEMENTS.lock().unwrap_or_else(|e| e.into_inner());
    if *p != names {
        *p = names;
    }
}

/// The candidates that start with what is typed, then those that contain it (case ignored).
fn matching(candidates: Vec<String>, prefix: &str) -> Vec<CommandSuggestion> {
    let typed = prefix.trim_start().to_lowercase();
    let mut starts: Vec<String> = Vec::new();
    let mut contains: Vec<String> = Vec::new();
    for c in candidates {
        let l = c.to_lowercase();
        if l.starts_with(&typed) {
            starts.push(c);
        } else if l.contains(&typed) {
            contains.push(c);
        }
    }
    starts.sort();
    contains.sort();
    starts.into_iter().chain(contains).take(50).map(CommandSuggestion::new).collect()
}

/// A schematic file of the schematics folder (the rest of the line: names may hold spaces).
struct Schematic;

impl CommandArgumentType for Schematic {
    fn kind(&self) -> CommandArgumentKind {
        CommandArgumentKind::Greedy
    }

    fn parse(&self, input: &str) -> Result<String, CommandArgumentError> {
        Ok(input.to_string())
    }

    fn suggest(&self, _sender: Option<&CommandSender>, prefix: &str) -> Vec<CommandSuggestion> {
        let dir = FOLDER.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let files = dir.map(|d| horizonmatica_core::list_schematics(&d)).unwrap_or_default();
        matching(files, prefix)
    }
}

/// A loaded placement, by number or name (the rest of the line).
struct Placement;

impl CommandArgumentType for Placement {
    fn kind(&self) -> CommandArgumentKind {
        CommandArgumentKind::Greedy
    }

    fn parse(&self, input: &str) -> Result<String, CommandArgumentError> {
        Ok(input.to_string())
    }

    fn suggest(&self, _sender: Option<&CommandSender>, prefix: &str) -> Vec<CommandSuggestion> {
        let names = PLACEMENTS.lock().unwrap_or_else(|e| e.into_inner()).clone();
        matching(names, prefix)
    }
}

/// Registers the argument types (from `Hooks::register`).
pub fn register(r: &mut dyn Registrar) {
    r.register_command_argument("schematic", Schematic);
    r.register_command_argument("placement", Placement);
}

/// The argument a subcommand's free part takes in its usage (`/hm load <file:...>`).
pub fn argument_of(sub: &str) -> &'static str {
    match sub {
        "load" => "<file:horizonmatica:schematic>",
        "select" | "unload" => "<placement:horizonmatica:placement>",
        _ => "<args:text>",
    }
}
