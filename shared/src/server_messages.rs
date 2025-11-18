use serde::{Deserialize, Serialize};

crate::message_definitions! {
    pub enum FromServer {
        opcode => ClientOpcode;
        Handshake(Handshake) = 0x8000;
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Handshake {
}
