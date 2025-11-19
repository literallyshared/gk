use server::{networking_core::{NetworkingCore}, nexus_core::NexusCore, realm::realm_core::RealmCore};
use simple_logger::SimpleLogger;

#[tokio::main]
async fn main() {
    SimpleLogger::new().env().init().unwrap();
    let networking_core = NetworkingCore::new().start("localhost".to_string(), 3310);
    let realm_core = RealmCore::new().start();
    let nexus_core = NexusCore::new().start(networking_core, realm_core);
    let (handle, _tx, _rx) = nexus_core.into_parts();
    let _ = handle.await;
}
