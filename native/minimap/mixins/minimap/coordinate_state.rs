thread_local! {
 pub static COORDINATES: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}
thread_local! {
 pub static OUTLINE: std::cell::Cell<(usize, bool)> = const { std::cell::Cell::new((0, false)) };
}

#[derive(Clone)]
pub struct Marker { pub x:f32, pub y:f32, pub face:Option<crate::hud::Rect>, pub hat:Option<crate::hud::Rect>, pub color:u32, pub label:String }
thread_local! {
 pub static MARKERS: std::cell::RefCell<Vec<Marker>> = const { std::cell::RefCell::new(Vec::new()) };
 pub static OWN_HEAD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
pub fn draw_markers(batch:&mut crate::hud::HudBatch, atlas:&crate::hud::HudAtlas, x:f32,y:f32,size:f32) {
 MARKERS.with_borrow(|markers| for m in markers {
  let (px,py)=(x+size/2.0+m.x,y+size/2.0+m.y);
  if let Some(face)=m.face { batch.rect_uv(px-4.0,py-4.0,8.0,8.0,face,u32::MAX); if let Some(hat)=m.hat {batch.rect_uv(px-4.5,py-4.5,9.0,9.0,hat,u32::MAX);} }
  else { batch.fill(atlas,px-2.0,py-2.0,4.0,4.0,m.color); if !m.label.is_empty() {batch.text(atlas,&m.label,px+3.0,py-3.0,m.color,true);} }
 });
}
