use uuid::Uuid;

#[allow(dead_code)]
pub enum SessionState {
    AwaitingLogin,
    Playing {
        entity_identifier: Uuid,
        account: String,
        display_name: String,
        current_map: String,
    },
}

impl SessionState {
    pub fn get_account(&self) -> Option<&String> {
        if let Self::Playing { account, .. } = self {
            return Some(account);
        }
        None
    }

    pub fn set_current_map(&mut self, new_map: String) {
        if let Self::Playing { account, current_map, .. } = self {
            info!("Updating current map for [{account}]: [{current_map}] -> [{new_map}]");
            *current_map = new_map;
        }
    }

    pub fn set_display_name(&mut self, new_display_name: String) {
        if let Self::Playing { account, display_name, .. } = self {
            info!("Updating display name for [{account}]: [{display_name}] -> [{new_display_name}]");
            *display_name = new_display_name;
        }
    }
}
