//! Exercise generation errors directly; never use a client's failed-chunk fallback.
use horizon_world::{level::data,worldgen::{generator::{GeneratorData,OverworldGenerator},overrides::WorldgenOverrides}};
use horizon_core::position::ChunkPos;
use std::{path::Path,collections::BTreeMap};
fn main()->Result<(),Box<dyn std::error::Error>>{
 let a:Vec<_>=std::env::args().collect();let root=Path::new(&a[1]);
 let registry=Box::leak(Box::new(data::load_registry(root)?));
 let mut ov=WorldgenOverrides::default();
 if let Some(id)=a.get(2).filter(|v|v.as_str()!="--natural"){let dimension=serde_json::json!({"type":"minecraft:overworld","generator":{"type":"minecraft:noise","settings":"minecraft:overworld","biome_source":{"type":"minecraft:fixed","biome":id}}});ov.add_file("probe","data/minecraft/dimension/overworld.json",||Some(dimension.to_string()));}
 let data=GeneratorData::load_with(&root.join("worldgen"),registry,&ov)?;
 let generator=OverworldGenerator::for_dimension(data,8448363262698333288,"minecraft:normal","minecraft:overworld")?;
 assert!(generator.unsupported_features().is_empty(),"unsupported: {:?}",generator.unsupported_features());
 let mut worker=generator.worker();
 if a.get(2).is_some_and(|v|v=="--natural"){
  let points:Vec<_>=(0..65536).flat_map(|i|[16,32,48].map(|y|[(i%256-128)*256,y,(i/256-128)*256])).collect();
  let mut counts=BTreeMap::new();for biome in worker.noise_biomes(&points).into_iter().flatten(){if biome.starts_with("horizon_gensokyo_legacy:"){*counts.entry(biome).or_insert(0usize)+=1;}}
  println!("NATURAL {counts:?}");assert_eq!(counts.len(),17,"every ported biome must be reachable");return Ok(())
 }

 let mut site=None;
 for cz in -16..=16{for cx in -16..=16{if let Some(s)=worker.surface_sample(cx*64+8,cz*64+8){if s.height>=70{site=Some((cx*4,cz*4));break}}}if site.is_some(){break}}
 let (cx,cz)=site.unwrap_or((0,0));let mut chunks=Vec::new();
 for z in -1..=1{for x in -1..=1{chunks.push(worker.generate_proto(ChunkPos::new(cx+x,cz+z))?);}}
 let mut region:[_;9]=chunks.try_into().ok().unwrap();let stats=worker.decorate(&mut region)?;
 assert_eq!(stats.unsupported,0,"unsupported roots: {:?}",stats.skipped);assert_eq!(stats.failed,0,"failed roots: {:?}",stats.skipped);
 let mut custom=BTreeMap::new();for chunk in &region{for y in -64..320{for z in 0..16{for x in 0..16{let name=chunk.block_name(x,y,z);if name.starts_with("horizon_gensokyo_legacy:"){*custom.entry(name.to_owned()).or_insert(0usize)+=1}}}}}
 println!("PASS {:?} site={cx},{cz} custom={custom:?} stats={stats:?}",a.get(2));
 Ok(())
}
