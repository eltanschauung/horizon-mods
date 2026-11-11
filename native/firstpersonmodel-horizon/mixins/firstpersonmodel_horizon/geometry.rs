// SPDX-License-Identifier: MIT
fn geometry(game: &crate::game::Game<'_>, camera: glam::DVec3, alpha:f64, out:&mut Vec<crate::mesher::Vertex>) -> core::ops::ControlFlow<()> { if !game.firstperson_body_active() { return core::ops::ControlFlow::Continue(()) } game.firstperson_body_geometry(camera,alpha,out); core::ops::ControlFlow::Break(()) }
