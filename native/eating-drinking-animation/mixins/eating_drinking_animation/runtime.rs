// SPDX-License-Identifier: MIT
use super::Game;
use horizon_client::{eating_drinking_animation as anim, mobs::{Mob, render}};
use horizon_world::consumable::UseAnimation;
impl Game<'_> {
 fn eating_drinking_kind(&self, stack:&horizon_client::items::ItemStack)->Option<bool> {
  let components=self.pack.item_renderer.models.components(&horizon_client::item_render::stack_view(stack));
  let mut a=components.get("minecraft:consumable").map_or(UseAnimation::None,horizon_client::first_person::consumable_animation);
  self.native_use_animation(stack,&mut a);
  match a {UseAnimation::Eat=>Some(false),UseAnimation::Drink=>Some(true),_=>None}
 }
 pub(super) fn eating_drinking_tick(&self) {
  let mut inputs=Vec::new();
  if self.native_on("eating_drinking_animation") && self.player.is_some() {
   for m in &self.mobs {
    if &*m.kind.id!="minecraft:player" {continue;}
    let active=(m.extra.using_item>0).then(|| {
     let stack=render::held_item_stack(&m.extra,m.extra.using_off_hand)?;
     let drink=self.eating_drinking_kind(&stack)?;
     Some((m.main_arm_left()!=m.extra.using_off_hand,drink,m.extra.using_item as f32))
    }).flatten();
    inputs.push((m.id,active));
   }
   if let Some(m)=&self.player_model {
    let active=self.using.and_then(|(slot,ticks)| {
     let drink=self.eating_drinking_kind(self.stack_in(slot)?)?;
     Some((self.settings.main_hand_left!=(slot==super::shield::OFFHAND_SLOT),drink,ticks as f32))
    });inputs.push((m.id,active));
   }
  }
  anim::update(&inputs);
 }
 pub(super) fn eating_drinking_particles(&mut self,item:&str,ticks:u32,at:[f64;3])->bool {
  if !self.native_on("eating_drinking_animation") {return false;}
  let Some(m)=self.mobs.iter().find(|m| &*m.kind.id=="minecraft:player" &&
   (m.position.x-at[0]).abs()<0.001 && (m.position.y-at[1]).abs()<0.001 && (m.position.z-at[2]).abs()<0.001) else {return false;};
  let Some(c)=self.items().get(item).and_then(|i|i.consumable.as_ref()) else{return false;};
  if !c.particles || !c.should_emit((c.ticks().max(0) as u32).saturating_sub(ticks) as i32) {return true;}
  // Camera-relative transform avoids losing precision far from world origin.
  let camera=glam::DVec3::from_array(at);
  let Some(t)=render::held_item_transform(m,1.0,camera,self.res.mob_data(),m.main_arm_left()!=m.extra.using_off_hand) else {return false;};
  let origin=t.transform_point3(glam::Vec3::ZERO).as_dvec3()+camera;
  self.eating_drinking_crumbs(&horizon_client::items::ItemStack::new(item,1),5,origin);
  true
 }
 pub(super) fn eating_drinking_local_particles(&mut self,stack:&horizon_client::items::ItemStack,count:u32)->bool {
  if !self.native_on("eating_drinking_animation") || self.camera_type.first_person() || self.using.is_none() {return false;}
  let Some(m)=&self.player_model else{return false;};
  let at=glam::DVec3::new(m.position.x,m.position.y,m.position.z);
  let left=self.settings.main_hand_left!=self.using.is_some_and(|(s,_)|s==super::shield::OFFHAND_SLOT);
  let Some(t)=render::held_item_transform(m,1.0,at,self.res.mob_data(),left) else{return false;};
  let origin=t.transform_point3(glam::Vec3::ZERO).as_dvec3()+at;
  self.eating_drinking_crumbs(stack,count,origin);true
 }
 fn eating_drinking_crumbs(&mut self,stack:&horizon_client::items::ItemStack,count:u32,origin:glam::DVec3) {
  let options=self.item_particle(stack);
  use horizon_core::random::RandomSource;
  for _ in 0..count {
   let jitter=std::array::from_fn::<_,3,_>(|_| f64::from(self.world.random.next_float())-0.5);
   let p=[origin.x+jitter[0]*0.12,origin.y+jitter[1]*0.12,origin.z+jitter[2]*0.12];
   self.particles.add(&options,p,[jitter[0]*0.04,0.02,jitter[2]*0.04]);
  }
 }
}
