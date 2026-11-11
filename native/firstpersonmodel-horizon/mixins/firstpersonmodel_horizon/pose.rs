// SPDX-License-Identifier: MIT
fn pose(_result: (), m: &Mob, p: &mut Poses) {
    if m.extra.hidden_model_parts & 128 != 0 { p.visible("head",false); p.visible("hat",false);
        if crate::mobs::anim::firstperson_view::arms_hidden() {
            for part in ["right_arm", "left_arm", "right_sleeve", "left_sleeve"] { p.visible(part, false); }
        } }
}
