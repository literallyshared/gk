use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NetEntityIdentifier {
    Player(Uuid),
    Npc(Uuid),
    Item(Uuid),
    Projectile(Uuid),
}
