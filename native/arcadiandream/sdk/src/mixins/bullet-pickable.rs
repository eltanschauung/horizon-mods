fn hook(p: &Projectile) -> core::ops::ControlFlow<bool> {
    if p.kind.id().starts_with("arcadiandream:") && p.kind.id().ends_with("_bullet") {
        core::ops::ControlFlow::Break(true)
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
