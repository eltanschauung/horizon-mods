// The voice chat's status in the top left corner (connecting, talking,
// muted, the group and who is talking), and each player's group above
// their head, seen from the camera the world was drawn from.
fn voicechat_hud(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    let _ = corners;
    use voicechat::engine as vc;
    if !game.on(vc::ID)
        || !game
            .remote
            .as_ref()
            .is_some_and(|r| r.joined && game.integrated.is_none() && r.reconfiguring.is_none())
    {
        return;
    }
    if let Some((pos, yaw, pitch, fov)) = game.locator_camera {
        let camera = vc::labels::Camera {
            pos,
            yaw,
            pitch,
            fov,
        };
        for label in vc::group_labels(&camera, [batch.width, batch.height]) {
            let w = game.native_text_width(batch, &label.text) + 4.0;
            let [x, y] = [label.at[0] - w / 2.0, label.at[1] - 6.0];
            game.native_fill(batch, [x, y, w, 12.0], 0x6000_0000);
            game.native_text(batch, &label.text, [x + 2.0, y + 2.0], label.rgb | 0xff00_0000, true);
        }
    }
}
