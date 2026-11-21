pub const ID: &str = "minimap";
pub const NAME: &str = "Minimap";
pub const VERSION: &str = "2.1.0";
pub const DESCRIPTION: &str = "A map of the terrain around you in a corner, in map colours, square or round, north up or turning with you. Horizon's own.";
/// The corners, as the settings files name them and the page labels them.
pub const CORNERS: [(&str, &str); 4] = [
    ("top_left", "Top left"),
    ("top_right", "Top right"),
    ("bottom_left", "Bottom left"),
    ("bottom_right", "Bottom right"),
];
/// Sides in GUI pixels, and blocks per map pixel.
pub const SIZES: [(u8, &str); 3] = [(64, "Small"), (96, "Medium"), (128, "Large")];
pub const ZOOMS: [f32; 4] = [0.5, 1.0, 2.0, 4.0];

/// The settings, as the WebAssembly Minimap kept them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// Indices in `CORNERS`, `SIZES` and `ZOOMS`.
    pub corner: usize,
    pub size: usize,
    pub round: bool,
    pub zoom: usize,
    pub rotate: bool,
    pub show_coordinates: bool,
    pub outline: usize,
    pub player_head: bool,
}

impl Settings {
    /// From the stored values; the extra's defaults (top right, medium,
    /// square, 1 block per pixel, north up) for the ones missing.
    pub fn read(get: impl Fn(&str) -> Option<String>) -> Self {
        let get = |key: &str| get(key).map(|v| v.trim().to_owned());
        let find = |key: &str, values: &[String], default: usize| {
            if let Some(index) = get(&format!("{key}_choice"))
                .and_then(|value| value.parse::<usize>().ok())
                .filter(|&index| index < values.len())
            {
                return index;
            }
            get(key)
                .and_then(|v| values.iter().position(|s| *s == v))
                .unwrap_or(default)
        };
        let corners: Vec<String> = CORNERS.iter().map(|c| c.0.to_owned()).collect();
        let sizes: Vec<String> = SIZES.iter().map(|s| s.0.to_string()).collect();
        let zooms: Vec<String> = ZOOMS.iter().map(f32::to_string).collect();
        Self {
            corner: find("corner", &corners, 1),
            size: find("size", &sizes, 0),
            round: find("shape", &["square".into(), "round".into()], 0) == 1,
            zoom: find("zoom", &zooms, 1),
            rotate: get("rotate").as_deref() == Some("true"),
            show_coordinates: get("show_coordinates").as_deref().unwrap_or("true") == "true",
            player_head: get("player_head").as_deref() == Some("true"),
            outline: find(
                "outline",
                &["black".into(), "stone".into(), "bamboo".into()],
                1,
            ),
        }
    }
    /// What Horizon's minimap draws: (side, blocks per pixel).
    pub fn view(&self) -> (f32, f32) {
        (f32::from(SIZES[self.size].0), ZOOMS[self.zoom])
    }
}

thread_local! {
    /// The settings, read once from the files, then kept.
    static SETTINGS: std::cell::Cell<Option<Settings>> = const { std::cell::Cell::new(None) };
}

/// The settings, read once through `get` (one file per key).
pub fn settings(get: impl Fn(&str) -> Option<String>) -> Settings {
    SETTINGS.with(|s| {
        let found = s.get().unwrap_or_else(|| Settings::read(get));
        s.set(Some(found));
        found
    })
}

/// Replaces the settings (the settings page changed them).
pub fn clear_settings() {
    SETTINGS.with(|s| s.set(None));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_options_keep_the_fork_defaults() {
        let settings = Settings::read(|_| None);
        assert_eq!((settings.corner, settings.size, settings.zoom), (1, 0, 1));
        assert!(!settings.round && !settings.rotate && !settings.player_head);
        assert!(settings.show_coordinates);
        assert_eq!(settings.outline, 1);
    }

    #[test]
    fn legacy_files_keep_their_meanings() {
        let settings = Settings::read(|key| {
            match key {
                "corner" => Some("bottom_left"),
                "size" => Some("128\n"),
                "shape" => Some("round"),
                "zoom" => Some("1"),
                "rotate" => Some("true"),
                "show_coordinates" => Some("false"),
                "outline" => Some("bamboo"),
                "player_head" => Some("true"),
                _ => None,
            }
            .map(str::to_owned)
        });
        assert_eq!((settings.corner, settings.size, settings.zoom), (2, 2, 1));
        assert!(settings.round && settings.rotate && settings.player_head);
        assert!(!settings.show_coordinates);
        assert_eq!(settings.outline, 2);
    }

    #[test]
    fn stored_sdk_choices_override_legacy_values() {
        let settings = Settings::read(|key| {
            match key {
                "corner" => Some("bottom_left"),
                "corner_choice" => Some("0"),
                "size" => Some("128"),
                "size_choice" => Some("9"),
                "zoom" => Some("1"),
                "zoom_choice" => Some("2"),
                _ => None,
            }
            .map(str::to_owned)
        });
        assert_eq!((settings.corner, settings.size, settings.zoom), (0, 2, 2));
    }
}
