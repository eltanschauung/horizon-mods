fn coordinates_before(corners: &mut Corners, look: &MinimapLook) {
    if !look.corner.top() && minimap::state::COORDINATES.with(|c| c.borrow().is_some()) {
        corners.place(look.corner, look.size, 7.0);
    }
}
