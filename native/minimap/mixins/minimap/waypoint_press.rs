fn handle(game:&crate::game::Game<'_>,screen:&crate::game::native_hooks::NativeScreenView<'_>,button:i32,canceled:&mut bool){ crate::game::minimap_waypoints::press(game,screen,button,canceled); }
