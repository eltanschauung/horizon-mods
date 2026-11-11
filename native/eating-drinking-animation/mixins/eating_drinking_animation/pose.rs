// SPDX-License-Identifier: MIT
fn pose(_result: (), m: &Mob, s:&State, p:&mut Poses) {
 if &*m.kind.id!="minecraft:player" || m.extra.spectator || !m.alive() {return;}
 let arms=crate::eating_drinking_animation::arms(m.join.get());
 let head=*p.get("head");
 for (side,a) in arms.iter().enumerate() {
  let weight=a.old+(a.weight-a.old)*s.partial.clamp(0.0,1.0);
  if weight<=0.0 {continue;}
  let w=weight*weight*(3.0-2.0*weight);
  let target=crate::eating_drinking_animation::rotations([head[3],head[4]],a.drink,a.ticks+s.partial,side==1);
  let arm=if side==1 {"left_arm"}else{"right_arm"};
  let q=p.get(arm);for i in 0..3 {q[i+3]+=(target[i]-q[i+3])*w;}
  // Sleeves are children of the arm: retain their local pose and inherit its transform.
 }
}
