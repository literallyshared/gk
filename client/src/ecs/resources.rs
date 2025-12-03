use bevy_ecs::prelude::*;
use macroquad::math::Rect;

use crate::map::Map;

#[derive(Resource)]
pub struct CurrentMap(pub Map);

#[derive(Resource, Default)]
pub struct Viewport(pub Rect);
