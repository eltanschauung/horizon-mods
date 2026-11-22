// SPDX-License-Identifier: MIT
fn opacity(original: f32, slot: &str) -> f32 {
    let opacity = mod_invis_armor::opacity(original, slot);
    if slot == "chestplate" {
        // The character skin layer must follow the same chestplate visibility as the armor.
        ::horizon_client_lib::mobs::render::set_chestplate_hides_skin(opacity >= 1.0);
    }
    opacity
}
