use crate::playing_state::PlayingState;

async fn start_offline_mode() -> Result<PlayingState> {
    let mut playing_state = PlayingState::new("offline_tutorial".to_string()).await?;
    playing_state.set_offline_mode();
    playing_state
}

async fn start_login() -> () {
}
