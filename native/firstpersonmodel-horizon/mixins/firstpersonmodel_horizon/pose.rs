// SPDX-License-Identifier: MIT
fn pose(_result: (), m: &Mob, p: &mut Poses) {
    if m.extra.hidden_model_parts & 128 != 0 { p.visible("head",false); p.visible("hat",false); }
}
