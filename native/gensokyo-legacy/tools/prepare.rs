//! Prepare imported assets and exercise inventory and harvesting through Horizon's readers.
use horizon_assets::{item_model::ClientItem, resolve::ModelLibrary};
use horizon_client_lib::{
    assets::Assets,
    biome::Biomes,
    mod_content::{self, ContentMod},
};
use horizon_core::random::LegacyRandom;
use horizon_items::{
    blocks::BlockFacts,
    items::{ItemStack, Items},
};
use std::{error::Error, path::Path};

const NS: &str = "horizon_gensokyo_legacy";
fn id(name: &str) -> String {
    format!("{NS}:{name}")
}

fn check_blocks(root: &Path, assets: &Assets, models: &ModelLibrary) -> Result<(), Box<dyn Error>> {
    let facts = BlockFacts::load(root, assets.registry)?;
    let items = Items::load(root, assets.registry, &facts, &Default::default())?;
    let stack = |item: &str| ItemStack {
        item: item.into(),
        count: 1,
        damage: 0,
        extra: None,
    };
    let roll = |name: &str, tool: Option<&str>| {
        let state = assets.registry.default_state(&id(name)).unwrap();
        items
            .block_drops(
                assets.registry,
                state,
                tool.map(stack).as_ref(),
                &mut LegacyRandom::new(42),
            )
            .into_iter()
            .map(|drop| drop.item.to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(roll("maple_log", None), [id("maple_log")]);
    assert_eq!(roll("jizo", Some("minecraft:wooden_pickaxe")), [id("jizo")]);
    assert_eq!(roll("flame_cattail", None), [id("flame_cattail")]);
    assert_eq!(roll("evergreen_vine", None), [id("evergreen_vine")]);
    assert!(roll("evergreen_vine_plant", None).is_empty());
    assert!(items.get(&id("evergreen_vine_plant")).is_none());
    assert_eq!(roll("broom_grass", None), [id("mystical_straw")]);
    assert_eq!(
        roll("broom_grass", Some("minecraft:shears")),
        [id("broom_grass")]
    );
    assert_eq!(
        roll("pink_cherry_leaves", Some("minecraft:shears")),
        [id("pink_cherry_leaves")]
    );
    assert_eq!(
        roll("pink_cherry_fallen_leaves", Some("minecraft:wooden_shovel")),
        [id("pink_cherry_fallen_leaves")]
    );
    assert!(roll("pink_cherry_fallen_leaves", None).is_empty());
    assert!(items.get(&id("pink_cherry_sapling")).is_some());

    let mut icons = 0;
    for entry in std::fs::read_dir(root.join(format!("assets/{NS}/items")))? {
        let path = entry?.path();
        let name = path.file_stem().unwrap().to_str().unwrap();
        assert!(items.get(&id(name)).is_some(), "unregistered item {name}");
        let definition: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
        ClientItem::decode(&definition)?;
        let model_id = format!("{NS}:item/{name}");
        assert!(models.resolves(&model_id), "missing item model {name}");
        let model = models.resolve(&model_id)?;
        assert!(
            model.uses_item_generator() || !model.elements().is_empty(),
            "empty item model {name}"
        );
        if definition["model"]["type"] == "minecraft:select" {
            let gui = format!("{model_id}_gui");
            assert!(models.resolves(&gui), "missing GUI model {name}");
            assert!(!models.resolve(&gui)?.elements().is_empty());
        }
        if name != "mystical_straw" {
            assert!(
                items.has_loot_table(&format!("{NS}:blocks/{name}")),
                "missing loot for {name}"
            );
        }
        icons += 1;
    }
    println!("PASS {icons} inventory models and block loot (hand, shovel, pickaxe and shears)");
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args().collect();
    let mut mods = Vec::new();
    for name in &a[3..] {
        let p = std::path::Path::new(name);
        let package = horizon_mods::Package::read(p)?;
        mods.push(ContentMod {
            id: package.manifest.id.clone(),
            content: horizon_mods::Content::read(p, &package.manifest)?,
        });
    }
    let root = mod_content::resource_root(
        std::path::Path::new(&a[1]),
        &mods,
        std::path::Path::new(&a[2]),
    )?;
    let assets = Assets::load(&root)?;
    let render = assets.reload(&root)?;
    let biomes = Biomes::load(&root)?;
    if mods.iter().any(|m| m.id == NS) {
        check_blocks(
            &root,
            &assets,
            render.model_library.as_ref().map_err(String::as_str)?,
        )?;
    }
    println!(
        "PASS assets/models biomes={} states={} root={}",
        biomes.list.len(),
        assets.registry.len(),
        root.display()
    );
    Ok(())
}
