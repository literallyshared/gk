use bevy_ecs::prelude::*;

use crate::ecs::resources::{CurrentMap, Viewport};

pub fn render_map(map: Res<CurrentMap>, viewport: Res<Viewport>) {
    map.0.draw(viewport.0);
}
