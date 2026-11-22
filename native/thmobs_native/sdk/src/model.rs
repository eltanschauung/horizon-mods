//! The original mods' Java models as Horizon model layers: `models_gen.rs` holds their parts and
//! boxes as `convert_models.py` read them, and this builds each into a `LayerDefinition`.

use mod_api::{CubeDeformation, CubeListBuilder, LayerDefinition, MeshDefinition, ModelPart, PartPose};

use crate::models_gen::MODELS;

pub struct Cube {
    pub tex: [i32; 2],
    pub origin: [f32; 3],
    pub size: [f32; 3],
    pub grow: f32,
    pub mirror: bool,
}

pub struct Part {
    pub name: &'static str,
    pub parent: Option<&'static str>,
    pub pivot: [f32; 3],
    pub rot: [f32; 3],
    pub cubes: &'static [Cube],
}

pub struct Model {
    /// `<mod>/<ModelClass>`.
    pub id: &'static str,
    pub tex: [u32; 2],
    pub parts: &'static [Part],
}

/// The layer a model is registered as: `thmobs_native:<mod>_<class>#main`.
pub fn layer_id(model: &str) -> String {
    format!("thmobs_native:{}#main", model.replace('/', "_").to_lowercase())
}

pub fn find(model: &str) -> Option<&'static Model> {
    MODELS.iter().find(|m| m.id == model)
}

/// A model's parts and boxes as a layer: each part hangs from its parent at its pivot, turned as the
/// Java model turned it; the parts the model drew itself hang from the root.
pub fn layer(model: &Model) -> LayerDefinition {
    let mut mesh = MeshDefinition::new();
    let root = mesh.root();
    let mut ids = Vec::new();
    for part in model.parts {
        let mut cubes = CubeListBuilder::create();
        for c in part.cubes {
            cubes = cubes.tex_offs(c.tex[0], c.tex[1]).set_mirror(c.mirror).add_box_with(c.origin, c.size, CubeDeformation::new(c.grow));
        }
        let parent = part.parent.and_then(|p| ids.iter().find(|(n, _)| *n == p).map(|(_, id)| *id)).unwrap_or(root);
        let [x, y, z] = part.pivot;
        let [rx, ry, rz] = part.rot;
        let id = mesh.add_or_replace_child(parent, part.name, cubes, PartPose::offset_and_rotation(x, y, z, rx, ry, rz));
        ids.push((part.name, id));
    }
    LayerDefinition::create(mesh, model.tex[0], model.tex[1])
}

/// Every model's layer.
pub fn layers() -> Vec<(String, LayerDefinition)> {
    MODELS.iter().map(|m| (layer_id(m.id), layer(m))).collect()
}

/// The part `name` anywhere in the tree.
pub fn part_mut<'a>(root: &'a mut ModelPart, name: &str) -> Option<&'a mut ModelPart> {
    if root.get_child(name).is_some() {
        return root.get_child_mut(name);
    }
    for (_, child) in root.children_mut() {
        if let Some(found) = part_mut(child, name) {
            return Some(found);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_model_has_parts_and_every_parent_comes_first() {
        assert!(MODELS.len() >= 30);
        for m in MODELS {
            assert!(!m.parts.is_empty(), "{}", m.id);
            for (i, p) in m.parts.iter().enumerate() {
                if let Some(parent) = p.parent {
                    assert!(m.parts[..i].iter().any(|q| q.name == parent), "{}: {} before {}", m.id, p.name, parent);
                }
            }
        }
    }

    #[test]
    fn the_fairy_is_as_its_java_model_made_it() {
        let fairy = find("kaguya/ModelTHFairy").unwrap();
        assert_eq!(fairy.tex, [64, 64]);
        let head = fairy.parts.iter().find(|p| p.name == "head").unwrap();
        assert_eq!(head.pivot, [0.0, 7.0, 0.0]);
        assert_eq!(head.cubes[0].size, [8.0, 8.0, 8.0]);
        let ribbon = fairy.parts.iter().find(|p| p.name == "ribbonCenter").unwrap();
        assert_eq!(ribbon.parent, Some("head"));
        assert!((ribbon.rot[0] - 0.2443461).abs() < 1e-6);
        assert_eq!(fairy.parts.iter().filter(|p| p.parent.is_none()).count(), 11);
    }

    #[test]
    fn layers_are_named_for_the_native_mod() {
        assert_eq!(layer_id("kaguya/ModelTHFairy"), "thmobs_native:kaguya_modelthfairy#main");
    }
}
