mod gk_shared_built_info {
   // The file has been placed there by the build script.
   include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

pub fn get_version() -> Option<String> {
    if let Some(hash) = gk_shared_built_info::GIT_VERSION {
        if let Some(dirty) = gk_shared_built_info::GIT_DIRTY {
            if dirty {
                return Some(String::from(hash) + "+");
            }
            return Some(String::from(hash));
        }
    }
    None
}
