fn handle(game:&crate::game::Game<'_>,screen:&crate::game::native_hooks::NativeScreenView<'_>,batch:&mut horizon_client::hud::HudBatch){ crate::game::minimap_waypoints::draw(game,screen,batch); }
