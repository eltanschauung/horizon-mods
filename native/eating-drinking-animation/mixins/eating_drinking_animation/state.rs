// SPDX-License-Identifier: MIT
use std::{cell::RefCell, collections::HashMap};
#[derive(Clone, Copy, Default)]
pub struct Arm { pub old: f32, pub weight: f32, pub drink: bool, pub ticks: f32 }
thread_local! { static ARMS: RefCell<HashMap<u32, [Arm; 2]>> = RefCell::new(HashMap::new()); }
pub fn update(inputs: &[(u32, Option<(bool, bool, f32)>)]) {
 ARMS.with(|all| {
  let mut all=all.borrow_mut();
  all.retain(|id,_| inputs.iter().any(|(i,_)| i==id));
  for &(id,active) in inputs {
   if active.is_none() && !all.contains_key(&id) {continue;}
   let arms=all.entry(id).or_default();
   for (side,arm) in arms.iter_mut().enumerate() {
    arm.old=arm.weight;
    let used=active.filter(|a| usize::from(a.0)==side);
    if let Some((_,drink,ticks))=used {arm.drink=drink;arm.ticks=ticks;}
    arm.weight=(arm.weight+if used.is_some(){0.2}else{-1.0/6.0}).clamp(0.0,1.0);
   }
  }
 });
}
pub fn arms(id:u32)->[Arm;2] { ARMS.with(|a|a.borrow().get(&id).copied().unwrap_or_default()) }
pub fn rotations(head:[f32;2], drink:bool, ticks:f32, left:bool)->[f32;3] {
 let side=if left {1.0}else{-1.0};
 let bob=(ticks*if drink {0.55}else{1.35}).sin()*if drink {0.025}else{0.065};
 [-1.65+head[0]*0.55+bob, head[1]+side*0.38, side*(0.13+bob*0.3)]
}
#[cfg(test)] mod tests {
 use super::*;
 #[test] fn raise_release_and_independent_hands() {
  update(&[]);for tick in 0..5 {update(&[(7,Some((false,false,tick as f32)))]);}
  assert_eq!(arms(7)[0].weight,1.0);assert_eq!(arms(7)[1].weight,0.0);
  update(&[(7,None)]);assert!(arms(7)[0].weight>0.0);assert_eq!(arms(7)[0].old,1.0);
  for _ in 0..6 {update(&[(7,None)]);}assert_eq!(arms(7)[0].weight,0.0);
  update(&[(7,Some((true,true,0.0)))]);assert!(arms(7)[1].weight>0.0);
  update(&[]);assert_eq!(arms(7)[1].weight,0.0);
 }
 #[test] fn bob_is_bounded_and_mirrored() {
  for tick in 0..200 {for drink in [false,true] {
   let r=rotations([0.0,0.0],drink,tick as f32,false);let l=rotations([0.0,0.0],drink,tick as f32,true);
   assert!((r[0]+1.65).abs()<0.07);assert_eq!(r[0],l[0]);assert_eq!(r[1],-l[1]);assert_eq!(r[2],-l[2]);
  }}
 }
}
