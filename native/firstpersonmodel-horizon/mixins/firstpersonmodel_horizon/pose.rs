// SPDX-License-Identifier: MIT
fn pose(_result: (), m: &Mob, p: &mut Poses) {
    if m.extra.hidden_model_parts & 128 != 0 {
        p.visible("head", false);
        p.visible("hat", false);
        if crate::firstperson_arm_visibility::hide_empty() {
            let main = if m.main_arm_left() { "left_arm" } else { "right_arm" };
            let off = if m.main_arm_left() { "right_arm" } else { "left_arm" };
            if m.extra.main_hand.is_none() { p.visible(main, false); }
            if m.extra.off_hand.is_none() { p.visible(off, false); }
        }
    }
}
