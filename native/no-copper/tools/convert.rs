//! Offline Anvil palette migration, using Horizon's own region and NBT codecs.
use horizon_world::{
    nbt::{Limits, Tag},
    region::{ChunkRecord, RegionFile},
};
use std::{
    fs, io,
    path::{Path, PathBuf},
};
fn bad(s: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, s)
}
#[derive(Default)]
struct Counts {
    copper: u64,
    deepslate: u64,
    sections: u64,
}
fn convert_sections(root: &mut Tag) -> io::Result<Counts> {
    let mut out = Counts::default();
    let sections = if root.get("sections").is_some() {
        root.get_mut("sections")
    } else {
        root.get_mut("Level").and_then(|v| v.get_mut("Sections"))
    };
    let Some(sections) = sections else {
        return Ok(out);
    };
    let Tag::List(sections) = sections else {
        return Err(bad("sections is not a list"));
    };
    for section in sections {
        let modern = section.get("block_states").is_some();
        let states = if modern {
            section.get_mut("block_states").unwrap()
        } else {
            section
        };
        let (palette_key, data_key) = if modern {
            ("palette", "data")
        } else {
            ("Palette", "BlockStates")
        };
        let Some(Tag::List(palette)) = states.get(palette_key) else {
            continue;
        };
        if palette.is_empty() || palette.len() > 4096 {
            return Err(bad("invalid block palette size"));
        }
        let kinds: Vec<u8> = palette
            .iter()
            .map(|entry| {
                match if matches!(entry, Tag::String(_)) {
                    Some(entry)
                } else {
                    entry.get("Name")
                } {
                    Some(Tag::String(s)) => match s.to_string_lossy().as_str() {
                        "minecraft:copper_ore" => 1,
                        "minecraft:deepslate_copper_ore" => 2,
                        _ => 0,
                    },
                    _ => 0,
                }
            })
            .collect();
        if !kinds.iter().any(|&k| k != 0) {
            continue;
        }
        let mut used = [0u64; 3];
        if palette.len() == 1 {
            used[kinds[0] as usize] = 4096;
        } else {
            let bits = (usize::BITS - (palette.len() - 1).leading_zeros()).max(4) as usize;
            let per = 64 / bits;
            let expected = 4096usize.div_ceil(per);
            let Some(Tag::LongArray(data)) = states.get(data_key) else {
                return Err(bad("missing packed block data"));
            };
            if data.len() != expected {
                return Err(bad(
                    "unsupported or corrupt block packing; expected padded 1.16+ data",
                ));
            }
            let mask = (1u64 << bits) - 1;
            for i in 0..4096 {
                let index = ((data[i / per] as u64 >> ((i % per) * bits)) & mask) as usize;
                let kind = *kinds
                    .get(index)
                    .ok_or_else(|| bad("palette index out of bounds"))?;
                used[kind as usize] += 1;
            }
        }
        out.copper += used[1];
        out.deepslate += used[2];
        out.sections += 1;
        let Some(Tag::List(palette)) = states.get_mut(palette_key) else {
            unreachable!()
        };
        for (entry, kind) in palette.iter_mut().zip(kinds) {
            let replacement = match kind {
                1 => "minecraft:coal_ore",
                2 => "minecraft:deepslate_coal_ore",
                _ => continue,
            };
            if matches!(entry, Tag::String(_)) {
                *entry = Tag::String(replacement.into());
            } else {
                *entry
                    .get_mut("Name")
                    .ok_or_else(|| bad("missing block name"))? = Tag::String(replacement.into());
            }
        }
    }
    Ok(out)
}
fn collect(root: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let p = entry.path();
        if ty.is_symlink() {
            return Err(bad("symlinks are not supported in the world tree"));
        }
        if ty.is_dir() {
            collect(&p, out)?;
        } else if p.extension().is_some_and(|s| s == "mca")
            && p.parent()
                .and_then(Path::file_name)
                .is_some_and(|s| s == "region")
        {
            out.push(p);
        }
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() < 3
        || !matches!(args[1].as_str(), "scan" | "stage")
        || (args[1] == "stage" && args.len() != 4)
    {
        return Err("usage: copper-to-coal scan WORLD | stage WORLD OUTPUT".into());
    }
    let world = Path::new(&args[2]).canonicalize()?;
    let dest = (args[1] == "stage").then(|| PathBuf::from(&args[3]));
    if let Some(d) = &dest {
        if d.exists() {
            return Err("output must not exist".into());
        }
        fs::create_dir_all(d)?;
    }
    let mut files = Vec::new();
    collect(&world, &mut files)?;
    files.sort();
    let mut chunks = 0u64;
    let mut changed_chunks = 0u64;
    let mut changed_regions = 0u64;
    let mut total = Counts::default();
    let mut changed_files = Vec::new();
    for path in &files {
        let mut region = RegionFile::open(path)?;
        let positions: Vec<_> = region.chunks().map(|(p, _)| p).collect();
        let mut changes = Vec::new();
        for pos in positions {
            let record = region
                .read(pos)?
                .ok_or_else(|| bad("missing listed chunk"))?;
            let mut nbt = record.nbt(Limits::default())?;
            chunks += 1;
            let count = convert_sections(&mut nbt.tag)?;
            if count.sections == 0 {
                continue;
            }
            total.copper += count.copper;
            total.deepslate += count.deepslate;
            total.sections += count.sections;
            changed_chunks += 1;
            if dest.is_some() {
                let converted = ChunkRecord::from_nbt(
                    &nbt,
                    record.compression,
                    record.timestamp,
                    Limits::default(),
                )?;
                let decoded = converted.nbt(Limits::default())?;
                if decoded != nbt {
                    return Err("NBT round-trip changed chunk data".into());
                }
                let mut audit = decoded;
                if convert_sections(&mut audit.tag)?.sections != 0 {
                    return Err("copper remained after conversion".into());
                }
                changes.push((pos, Some(converted)));
            }
        }
        if !changes.is_empty() {
            changed_regions += 1;
            let relative = path.strip_prefix(&world)?;
            let target = dest.as_ref().unwrap().join(relative);
            fs::create_dir_all(target.parent().unwrap())?;
            region.export(&target, &changes)?;
            changed_files.push(relative.to_string_lossy().to_string());
        }
    }
    println!(
        "{}",
        serde_json::json!({"regions_scanned":files.len(),"chunks_scanned":chunks,"copper_ore":total.copper,"deepslate_copper_ore":total.deepslate,"sections_changed":total.sections,"chunks_changed":changed_chunks,"regions_written":changed_regions,"files":changed_files})
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn state(name: &str) -> Tag {
        Tag::compound([("Name", Tag::String(name.into()))])
    }
    #[test]
    fn compact_string_palette_is_converted() {
        let mut root = Tag::compound([(
            "sections",
            Tag::List(vec![Tag::compound([(
                "block_states",
                Tag::compound([(
                    "palette",
                    Tag::List(vec![Tag::String("minecraft:deepslate_copper_ore".into())]),
                )]),
            )])]),
        )]);
        let count = convert_sections(&mut root).unwrap();
        assert_eq!(count.deepslate, 4096);
        assert_eq!(convert_sections(&mut root).unwrap().sections, 0);
    }
    #[test]
    fn single_palette_and_unrelated_data_are_preserved() {
        let mut root = Tag::compound([
            (
                "sections",
                Tag::List(vec![Tag::compound([
                    (
                        "block_states",
                        Tag::compound([(
                            "palette",
                            Tag::List(vec![state("minecraft:copper_ore")]),
                        )]),
                    ),
                    ("sentinel", Tag::String("unchanged".into())),
                ])]),
            ),
            (
                "inventory",
                Tag::List(vec![Tag::String("minecraft:copper_ore".into())]),
            ),
        ]);
        let original_inventory = root.get("inventory").cloned();
        let result = convert_sections(&mut root).unwrap();
        assert_eq!(result.copper, 4096);
        assert_eq!(root.get("inventory"), original_inventory.as_ref());
        assert_eq!(convert_sections(&mut root).unwrap().sections, 0);
    }
    #[test]
    fn duplicate_coal_palette_indices_and_deepslate_are_preserved() {
        let data: Vec<i64> = (0..256).map(|_| 0x3210321032103210).collect();
        let mut root = Tag::compound([(
            "sections",
            Tag::List(vec![Tag::compound([(
                "block_states",
                Tag::compound([
                    (
                        "palette",
                        Tag::List(vec![
                            state("minecraft:stone"),
                            state("minecraft:copper_ore"),
                            state("minecraft:coal_ore"),
                            state("minecraft:deepslate_copper_ore"),
                        ]),
                    ),
                    ("data", Tag::LongArray(data.clone())),
                ]),
            )])]),
        )]);
        let result = convert_sections(&mut root).unwrap();
        assert_eq!(result.copper, 1024);
        assert_eq!(result.deepslate, 1024);
        let Some(Tag::List(s)) = root.get("sections") else {
            panic!()
        };
        assert_eq!(
            s[0].get("block_states").unwrap().get("data"),
            Some(&Tag::LongArray(data))
        );
    }
}
