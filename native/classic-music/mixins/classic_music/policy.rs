// SPDX-License-Identifier: MIT
pub const ID: &str = "classic_music";
pub const VERSION: &str = "0.1.0";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pool {
    Menu,
    Overworld,
    Nether,
    End,
    Dragon,
    Credits,
}
pub const POOLS: [Pool; 6] = [
    Pool::Menu,
    Pool::Overworld,
    Pool::Nether,
    Pool::End,
    Pool::Dragon,
    Pool::Credits,
];

pub fn group(event: &str) -> Option<Pool> {
    let name = event.strip_prefix("minecraft:")?;
    match name {
        "music.menu" => Some(Pool::Menu),
        "music.end" => Some(Pool::End),
        "music.dragon" => Some(Pool::Dragon),
        "music.credits" => Some(Pool::Credits),
        _ if name == "music.nether" || name.starts_with("music.nether.") => Some(Pool::Nether),
        _ if name.starts_with("music.") => Some(Pool::Overworld),
        _ => None,
    }
}
impl Pool {
    pub fn name(self) -> &'static str {
        match self {
            Self::Menu => "menu",
            Self::Overworld => "overworld",
            Self::Nether => "nether",
            Self::End => "end",
            Self::Dragon => "dragon",
            Self::Credits => "credits",
        }
    }
    pub fn tracks(self) -> &'static [&'static str] {
        match self {
            Self::Menu => &[
                "music/menu/beginning_2",
                "music/menu/floating_trees",
                "music/menu/moog_city_2",
                "music/menu/mutation",
            ],
            Self::Overworld => &[
                "music/game/clark",
                "music/game/danny",
                "music/game/dry_hands",
                "music/game/haggstrom",
                "music/game/key",
                "music/game/living_mice",
                "music/game/mice_on_venus",
                "music/game/minecraft",
                "music/game/oxygene",
                "music/game/subwoofer_lullaby",
                "music/game/sweden",
                "music/game/wet_hands",
                "music/game/creative/aria_math",
                "music/game/creative/biome_fest",
                "music/game/creative/blind_spots",
                "music/game/creative/dreiton",
                "music/game/creative/haunt_muskie",
                "music/game/creative/taswell",
                "music/game/water/axolotl",
                "music/game/water/dragon_fish",
                "music/game/water/shuniji",
            ],
            Self::Nether => &[
                "music/game/nether/concrete_halls",
                "music/game/nether/dead_voxel",
                "music/game/nether/ballad_of_the_cats",
                "music/game/nether/warmth",
            ],
            Self::End => &["music/game/end/the_end"],
            Self::Dragon => &["music/game/end/boss"],
            Self::Credits => &["music/game/end/alpha"],
        }
    }
    pub fn allows(self, file: &str) -> bool {
        file.strip_prefix("minecraft:")
            .is_some_and(|p| self.tracks().contains(&p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_lists_and_dimension_boundaries() {
        assert_eq!(Pool::Overworld.tracks().len(), 21);
        assert_eq!(Pool::Menu.tracks().len(), 4);
        assert_eq!(Pool::Nether.tracks().len(), 4);
        let unique: std::collections::HashSet<_> = Pool::Overworld.tracks().iter().collect();
        assert_eq!(unique.len(), 21);
        for event in [
            "minecraft:music.game",
            "minecraft:music.creative",
            "minecraft:music.under_water",
            "minecraft:music.overworld.swamp",
        ] {
            assert_eq!(group(event), Some(Pool::Overworld));
        }
        assert_eq!(
            group("minecraft:music.nether.crimson_forest"),
            Some(Pool::Nether)
        );
        assert!(!Pool::Menu.allows("minecraft:music/game/clark"));
        assert!(!Pool::Nether.allows("minecraft:music/game/nether/nether_wastes/rubedo"));
        assert!(Pool::Nether.allows("minecraft:music/game/nether/warmth"));
        assert!(!Pool::Overworld.allows("minecraft:music/game/comforting_memories"));
    }
    #[test]
    fn discs_and_other_mods_are_not_background_music() {
        for event in [
            "minecraft:music_disc.pigstep",
            "minecraft:music_disc.otherside",
            "minecraft:music_disc.cat",
            "lunatic:music.example",
            "minecraft:ambient.cave",
        ] {
            assert_eq!(group(event), None);
        }
        assert_eq!(group("minecraft:music.end"), Some(Pool::End));
    }
}
