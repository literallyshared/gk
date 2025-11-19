use tokio::task::JoinHandle;
use tokio::sync::mpsc;

pub struct Core<Command, Event = ()> {
    pub tx: tokio::sync::mpsc::UnboundedSender<Command>,
    pub rx: Option<tokio::sync::mpsc::UnboundedReceiver<Event>>,
    pub handle: tokio::task::JoinHandle<()>,
}

impl<Command, Event> Core<Command, Event> {
    pub fn new(
        tx: mpsc::UnboundedSender<Command>,
        handle: JoinHandle<()>,
    ) -> Self {
        Self {
            tx,
            rx: None,
            handle,
        }
    }

    pub fn with_events(mut self, rx: tokio::sync::mpsc::UnboundedReceiver<Event>) -> Self {
        self.rx = Some(rx);
        self
    }

    pub fn into_parts(self) -> (JoinHandle<()>, mpsc::UnboundedSender<Command>, Option<mpsc::UnboundedReceiver<Event>>) {
        (self.handle, self.tx, self.rx)
    }

    pub fn take_rx(&mut self) -> Option<mpsc::UnboundedReceiver<Event>> {
        self.rx.take()
    }

    pub async fn stop(self, stop_command: Option<Command>) -> Result<(), tokio::task::JoinError> {
        if let Some(command) = stop_command {
            let _ = self.tx.send(command);
        }
        self.handle.await?;
        Ok(())
    }
}
