use std::collections::HashMap;

use bevy_ecs::prelude::*;

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
        for (position, collider, _) in entities {
        }
    }
}
