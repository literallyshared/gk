use std::collections::HashMap;

use bevy_ecs::prelude::*;
use parry2d::bounding_volume::Aabb;
use shared::collision::Grid;

use crate::realm::ecs::components::{Collider, CurrentMap, Position};

pub fn resolve_collisions(
    query: Query<(Entity, &mut Position, &Collider, &CurrentMap)>,
) {
    // TODO: filter by current map, only check entities against each other if they are in the same
    // map

    let mut per_map: HashMap<&CurrentMap, Vec<(Entity, &Position, &Collider)>> = HashMap::default();
    for (entity, position, collider, map) in query.iter() {
        per_map.entry(map).or_default().push((entity, position, collider));
    }
    for (_, entities) in per_map {
        let mut grid: Grid<Entity> = Grid::new(10.0);
        let mut aabbs = HashMap::default();
        for (entity, position, collider) in entities {
            let mins = parry2d::na::point!(position.x - collider.w / 2.0, position.y - collider.h / 2.0);
            let maxs = parry2d::na::point!(position.x + collider.w / 2.0, position.y + collider.h / 2.0);
            aabbs.insert(entity, Aabb::new(mins, maxs));
        }
    }
}
