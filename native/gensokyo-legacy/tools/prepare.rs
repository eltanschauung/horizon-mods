//! Prepare imported assets independently of the generator test binary.
use horizon_client_lib::{assets::Assets,biome::Biomes,mod_content::{self,ContentMod}};
fn main()->Result<(),Box<dyn std::error::Error>>{
 let a:Vec<_>=std::env::args().collect();let mut mods=Vec::new();
 for name in &a[3..]{let p=std::path::Path::new(name);let package=horizon_mods::Package::read(p)?;mods.push(ContentMod{id:package.manifest.id.clone(),content:horizon_mods::Content::read(p,&package.manifest)?});}
 let root=mod_content::resource_root(std::path::Path::new(&a[1]),&mods,std::path::Path::new(&a[2]))?;
 let assets=Assets::load(&root)?;assets.reload(&root)?;let biomes=Biomes::load(&root)?;
 println!("PASS assets/models biomes={} states={} root={}",biomes.list.len(),assets.registry.len(),root.display());
 Ok(())
}
