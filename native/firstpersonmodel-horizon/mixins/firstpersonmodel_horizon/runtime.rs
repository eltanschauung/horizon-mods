// SPDX-License-Identifier: MIT
use super::Game;
thread_local! { static VANILLA_HANDS: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) }; }
thread_local! { static AUTO_HIDE: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) }; }
pub(super) const ID: &str = "firstpersonmodel_horizon";
impl Game<'_> {
    pub(super) fn firstperson_vanilla_hands(&self) -> bool {
        VANILLA_HANDS.with(|cached| {
            if let Some(value) = cached.get() { return value; }
            let value = self.native_setting(ID, "vanilla_hands").as_deref() != Some("false");
            cached.set(Some(value));
            value
        })
    }
    pub(super) fn firstperson_toggle_hands(&self) {
        let value = !self.firstperson_vanilla_hands();
        self.native_set_setting(ID, "vanilla_hands", if value { "true" } else { "false" });
        VANILLA_HANDS.with(|cached| cached.set(Some(value)));
    }
    pub(super) fn firstperson_auto_hide(&self) -> bool {
        AUTO_HIDE.with(|cached| {
            if let Some(value) = cached.get() { return value; }
            let value = self.native_setting(ID, "auto_hide_body").as_deref() != Some("false");
            cached.set(Some(value));
            value
        })
    }
    pub(super) fn firstperson_toggle_auto_hide(&self) {
        let value = !self.firstperson_auto_hide();
        self.native_set_setting(ID, "auto_hide_body", if value { "true" } else { "false" });
        AUTO_HIDE.with(|cached| cached.set(Some(value)));
    }
    pub(super) fn firstperson_body_active(&self) -> bool {
        self.native_on(ID) && self.camera_type.first_person() && !self.spectator()
            && self.sleeping.is_none() && self.studio_replay.is_none()
            && self.player.as_ref().is_some_and(|p| !p.health.dead_or_dying())
            && !(self.firstperson_auto_hide()
                && (self.pitch > 80.0 || self.player.as_ref().is_some_and(|p| p.swimming)))
            && (self.firstperson_vanilla_hands() || !self.visible_hand_items().iter().flatten().any(|s|
                matches!(s.item.as_ref(), "minecraft:filled_map" | "minecraft:spyglass")))
    }
    pub(super) fn firstperson_body_geometry(&self, camera: glam::DVec3, alpha: f64,
        out: &mut Vec<horizon_client::mesher::Vertex>) {
        let Some(original) = self.player_model.as_ref() else { return };
        let mut m = original.clone();
        // A render-copy-only marker: never changes remote players, inventory
        // previews, player dimensions, position packets or simulation state.
        m.extra.hidden_model_parts |= 128;
        let scale = self.player.as_ref().map_or(1.0, |p| f64::from(p.scale));
        let yaw = (f64::from(m.previous_body_yaw) +
            f64::from(((m.body_yaw - m.previous_body_yaw + 180.0).rem_euclid(360.0) - 180.0)) * alpha).to_radians();
        let distance = if m.extra.crouching { 0.35 } else { 0.25 } * scale;
        let offset = glam::DVec3::new(distance * yaw.sin(), 0.0, -distance * yaw.cos());
        let camera = camera - offset;
        let probe = [m.position.x.floor() as i32, (m.position.y + self.player.as_ref().map_or(1.62,|p|f64::from(p.dimensions.eye_height))).floor() as i32,m.position.z.floor() as i32];
        let light = self.world.light(probe[0],probe[1],probe[2]);
        let packed = (u32::from(light >> 4)*16) | ((u32::from(light & 15)*16)<<8);
        let atlas = &self.gui.atlas;
        let lookup = |name: &str| atlas.sprites.get(name).map(|r|[r.u0,r.v0,r.u1,r.v1]);
        let render = if self.own_body_visible() { horizon_client::mobs::render::render_with }
            else { horizon_client::mobs::render::render_layers };
        horizon_client::firstperson_arm_visibility::with_empty_hidden(self.firstperson_auto_hide(), || {
            render(&m,alpha as f32,camera,&lookup,packed,self.res.mob_data(),&self.player_armor_layers(),out);
        });
        if horizon_client::mobs::render::layers_shown(&m) {
            let chest = self.armor[1].as_ref().map(|s| &*s.item);
            if !self.own_invisible() { self.player_cape_geometry(&m,chest,camera,alpha as f32,packed,out); }
            self.player_elytra_geometry(&m,chest,camera,alpha as f32,packed,out);
            if !self.firstperson_vanilla_hands() { self.held_item_geometry(&m,camera,alpha,packed,true,None,out); }
        }
    }
}
