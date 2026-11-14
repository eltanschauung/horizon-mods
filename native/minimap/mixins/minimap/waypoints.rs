use super::{Game, Screen};
use horizon_client::{hud::{HudBatch, Rect}, text::Text, ui};
use std::cell::RefCell;
#[derive(Clone,serde::Serialize,serde::Deserialize)]
struct Waypoint { name:String, position:[f64;3], dimension:String, color:usize, enabled:bool }
const COLORS:[u32;6]=[0xff5555ff,0xff55ff55,0xffffaa00,0xffff55ff,0xff55ffff,0xffffffff];
#[derive(Default)]
struct State { scope:String, points:Vec<Waypoint>, writable:bool, open:bool, selected:Option<usize>, form:[String;4], field:usize, replace_on_type:bool, color:usize, page:usize, rects:Vec<[f32;4]>, error:String }
thread_local! { static STATE:RefCell<State> = RefCell::new(State::default()); }
fn scope(game:&Game<'_>)->String {
 let source=if game.integrated.is_none() { game.server_address().map(str::to_owned) } else { None }
     .unwrap_or_else(||game.save.root.to_string_lossy().into_owned());
 let hash=source.as_bytes().iter().fold(0xcbf29ce484222325u64,|h,b|(h^u64::from(*b)).wrapping_mul(0x100000001b3));
 format!("waypoints-{hash:016x}.json")
}
fn load(game:&Game<'_>,s:&mut State){
 let key=scope(game);if s.scope==key{return;}
 let data=game.native_setting(super::official_minimap::ID,&key);
 let parsed=data.as_deref().map(serde_json::from_str::<Vec<Waypoint>>);
 let writable=parsed.as_ref().is_none_or(|r|r.as_ref().is_ok_and(|v|v.len()<=512 && v.iter().all(|p|p.name.chars().count()<=64 && p.position.iter().all(|v|v.is_finite()&&v.abs()<=30_000_000.0))));
 let points=parsed.and_then(Result::ok).filter(|v|writable && v.len()<=512).unwrap_or_default();
 *s=State {scope:key,points,writable,..State::default()};
 if !writable{s.error="Cannot read waypoint file; original preserved".into();}
}
fn save(game:&Game<'_>,s:&mut State){
 if !s.writable{return;}
 let path=game.native_data_dir(super::official_minimap::ID).join(&s.scope);
 let result=serde_json::to_vec(&s.points).ok().and_then(|bytes|horizon_client::save::write_durable(&path,&bytes).ok());
 if result.is_none(){s.error="Could not save waypoints; previous file preserved".into();}
}

fn new_point(game:&Game<'_>,s:&mut State){s.selected=None;s.field=0;s.replace_on_type=true;s.form=["Waypoint".into(),"0".into(),"64".into(),"0".into()];if let Some(p)=game.native_player(){s.form[1]=format!("{:.0}",p.x);s.form[2]=format!("{:.0}",p.y);s.form[3]=format!("{:.0}",p.z);}}
pub(super) fn open_key(game:&mut Game<'_>,key:&str,pressed:bool){
 if !pressed || key!=game.native_mod_key(super::official_minimap::ID,"waypoints_key","key.keyboard.u") || !game.native_on(super::official_minimap::ID) || game.screen!=Screen::Playing{return;}
 STATE.with_borrow_mut(|s|{load(game,s);s.open=true;new_point(game,s);});
 game.open_options(false);game.options_to_game=true;game.capture(false);
}
fn active(s:&State,screen:&super::native_hooks::NativeScreenView<'_>)->bool{s.open && screen.id=="options"}
pub(super) fn draw(game:&Game<'_>,screen:&super::native_hooks::NativeScreenView<'_>,batch:&mut HudBatch){
 STATE.with_borrow_mut(|s|{
 if !active(s,screen){return;}
 let a=&game.gui.atlas;batch.fill(a,0.0,0.0,batch.width,batch.height,0xff161616);
 let w=(batch.width-16.0).clamp(100.0,340.0);let x=(batch.width-w)/2.0;let y=18.0;
 batch.text(a,"Waypoints",x,y-12.0,u32::MAX,true);s.rects.clear();
 let rows=5;let from=s.page*rows;
 for row in 0..rows {let i=from+row;let label=s.points.get(i).map(|p|format!("{} {} [{}]",if p.enabled{"●"}else{"○"},p.name.chars().take(14).collect::<String>(),p.dimension.trim_start_matches("minecraft:").chars().take(6).collect::<String>())).unwrap_or_else(||"-".into());let rect=[x,y+row as f32*21.0,w*0.54,20.0];s.rects.push(rect);ui::button(batch,a,rect[0],rect[1],rect[2],rect[3],&Text::literal(label),i<s.points.len(),s.selected==Some(i));}
 let fy=y;let fx=x+w*0.54+4.0;
 for i in 0..4 {let rect=[fx,fy+i as f32*21.0,w*0.46-4.0,20.0];s.rects.push(rect);ui::button(batch,a,rect[0],rect[1],rect[2],rect[3],&Text::literal(format!("{}: {}{}",["Name","X","Y","Z"][i],s.form[i].chars().rev().take(13).collect::<String>().chars().rev().collect::<String>(),if s.field==i{"_"}else{""})),true,s.field==i);}
 let labels=["New","Save","Delete","Toggle","Color","<",">"];
 for (i,label) in labels.iter().enumerate(){let bw=w/7.0;let rect=[x+i as f32*bw,fy+110.0,bw-1.0,20.0];s.rects.push(rect);ui::button(batch,a,rect[0],rect[1],rect[2],rect[3],&Text::literal(*label),s.writable,false);}
 batch.text(a,"Esc: close / Tab: field",x,fy+134.0,COLORS[s.color%6],true);
 if !s.error.is_empty(){batch.text(a,&s.error,x,fy+146.0,0xff5555ff,true);}
 });
}
fn hit(r:[f32;4],p:[f32;2])->bool{p[0]>=r[0]&&p[0]<r[0]+r[2]&&p[1]>=r[1]&&p[1]<r[1]+r[3]}
pub(super) fn press(game:&Game<'_>,screen:&super::native_hooks::NativeScreenView<'_>,button:i32,canceled:&mut bool){
 STATE.with_borrow_mut(|s|{if !active(s,screen){return;}*canceled=true;if button!=0{return;}
 let Some(i)=s.rects.iter().position(|r|hit(*r,screen.mouse)) else{return;};
 if i<5{let idx=s.page*5+i;if let Some(p)=s.points.get(idx){s.selected=Some(idx);s.form=[p.name.clone(),p.position[0].to_string(),p.position[1].to_string(),p.position[2].to_string()];s.color=p.color%6;s.replace_on_type=true;}return;}
 if i<9{s.field=i-5;s.replace_on_type=true;return;}
 match i-9 {
 0=>new_point(game,s),
 1=>{let coords=s.form[1..].iter().map(|v|v.parse::<f64>()).collect::<Result<Vec<_>,_>>();if let Ok(c)=coords && c.iter().all(|v|v.is_finite()&&v.abs()<=30_000_000.0) && !s.form[0].trim().is_empty() && s.writable {
 let p=Waypoint{name:s.form[0].trim().into(),position:[c[0],c[1],c[2]],dimension:game.dim.id().into(),color:s.color,enabled:true};if let Some(i)=s.selected{s.points[i]=p;}else if s.points.len()<512{s.points.push(p);s.selected=Some(s.points.len()-1);}s.error.clear();save(game,s);}else{s.error="Enter a name and valid coordinates".into();}},
 2=>{if let Some(i)=s.selected.take(){if s.writable{s.points.remove(i);save(game,s);}}},
 3=>{if let Some(i)=s.selected{if s.writable{s.points[i].enabled=!s.points[i].enabled;save(game,s);}}},
 4=>s.color=(s.color+1)%6,
 5=>s.page=s.page.saturating_sub(1),
 6=>s.page=(s.page+1).min(s.points.len().saturating_sub(1)/5),_=>{}
 }
 });
}
pub(super) fn input(_game:&Game<'_>,screen:&super::native_hooks::NativeScreenView<'_>,key:&str,modifiers:u32,canceled:&mut bool){STATE.with_borrow_mut(|s|{if !active(s,screen){return;}if key.ends_with("escape"){s.open=false;return;}*canceled=true;if key.ends_with("a") && modifiers & 2 != 0 {s.replace_on_type=true;}if key.ends_with("tab"){s.field=(s.field+1)%4;s.replace_on_type=true;}if key.ends_with("backspace"){let f=s.field;if s.replace_on_type{s.form[f].clear();s.replace_on_type=false;}else{s.form[f].pop();}}});}
pub(super) fn typed(_game:&Game<'_>,screen:&super::native_hooks::NativeScreenView<'_>,character:char,canceled:&mut bool){STATE.with_borrow_mut(|s|{if active(s,screen){*canceled=true;let f=s.field;if !character.is_control(){if s.replace_on_type{s.form[f].clear();s.replace_on_type=false;}if s.form[f].chars().count()<64{s.form[f].push(character);}}}});}
pub(super) fn swallow(_game:&Game<'_>,screen:&super::native_hooks::NativeScreenView<'_>,canceled:&mut bool){STATE.with_borrow(|s|if active(s,screen){*canceled=true;});}
fn skin_rects(game:&Game<'_>,skin:&str)->Option<(Rect,Rect)>{let r=*game.gui.atlas.sprites.get(skin)?;let sub=|x:f32,y:f32|Rect{u0:r.u0+(r.u1-r.u0)*x/64.0,v0:r.v0+(r.v1-r.v0)*y/64.0,u1:r.u0+(r.u1-r.u0)*(x+8.0)/64.0,v1:r.v0+(r.v1-r.v0)*(y+8.0)/64.0,w:8,h:8};Some((sub(8.0,8.0),sub(40.0,8.0)))}
pub(super) fn markers(game:&Game<'_>,settings:super::official_minimap::Settings){
 use horizon_client::extras_hud::minimap_coordinates::{MARKERS,OWN_HEAD,Marker};
 let Some(p)=game.native_player()else{return;};let (size,bpp)=settings.view();let reach=size/2.0-7.0;let yaw=game.yaw as f64;let (sin,cos)=yaw.to_radians().sin_cos();
 let project=|x:f64,z:f64,clamp:bool|{let (dx,dz)=((x-p.x)/f64::from(bpp),(z-p.z)/f64::from(bpp));let (x,y)=if settings.rotate{(-cos*dx-sin*dz,sin*dx-cos*dz)}else{(dx,dz)};let (mut x,mut y)=(x as f32,y as f32);let d=if settings.round{x.hypot(y)}else{x.abs().max(y.abs())};if d>reach {if !clamp{return None;}x*=reach/d;y*=reach/d;}Some((x,y))};
 let mut markers=Vec::new();
 for m in game.mobs.iter().filter(|m|m.kind.id.as_str()=="minecraft:player" && !m.is_invisible()) {if let Some((x,y))=project(m.position.x,m.position.z,false) && let Some((face,hat))=m.skin.as_deref().and_then(|skin|skin_rects(game,skin)){let opacity=game.player_presentation_opacity(m,glam::DVec3::new(p.x,p.y,p.z)).clamp(0.0,1.0);let color=(((opacity*255.0).round() as u32)<<24)|0x00ffffff;markers.push(Marker{x,y,face:Some(face),hat:Some(hat),color,label:String::new()});}}
 if settings.player_head && let Some((face,hat))=game.player_model.as_ref().and_then(|m|m.skin.as_deref()).and_then(|skin|skin_rects(game,skin)){markers.push(Marker{x:0.0,y:0.0,face:Some(face),hat:Some(hat),color:u32::MAX,label:String::new()});OWN_HEAD.with(|v|v.set(true));}
 STATE.with_borrow_mut(|s|{load(game,s);for w in s.points.iter().filter(|w|w.enabled&&w.dimension==game.dim.id()){if let Some((x,y))=project(w.position[0],w.position[2],true){markers.push(Marker{x,y,face:None,hat:None,color:COLORS[w.color%6],label:w.name.chars().take(1).collect()});}}});
 MARKERS.with_borrow_mut(|v|*v=markers);
}
