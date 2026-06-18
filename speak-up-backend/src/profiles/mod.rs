use speak_up_core::{AppContext, Profile};
use std::collections::HashMap;

#[allow(dead_code)]
pub struct ProfileManager {
    profiles: HashMap<String, Profile>,
    rules: Vec<ProfileRule>,
}

#[allow(dead_code)]
struct ProfileRule {
    app_pattern: String,
    profile_name: String,
}

impl Default for ProfileManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ProfileManager {
    pub fn new() -> Self {
        let mut profiles = HashMap::new();
        profiles.insert(
            "generic".into(),
            Profile {
                name: "generic".into(),
                system_prompt_template: "Clean up the transcript naturally.".into(),
                client_post_process: vec![],
            },
        );
        profiles.insert(
            "email".into(),
            Profile {
                name: "email".into(),
                system_prompt_template: "Format as a professional email. Add appropriate salutation and closing if the transcript suggests one.".into(),
                client_post_process: vec![],
            },
        );
        profiles.insert(
            "code".into(),
            Profile {
                name: "code".into(),
                system_prompt_template: "Preserve code identifiers and syntax exactly as spoken. Add line breaks only where indicated. Do not fix code syntax.".into(),
                client_post_process: vec![],
            },
        );
        Self { profiles, rules: Vec::new() }
    }

    pub fn match_profile(&self, _context: &AppContext) -> Option<&Profile> {
        None
    }

    pub fn get_fallback(&self) -> Profile {
        self.profiles
            .get("generic")
            .cloned()
            .unwrap_or_else(|| Profile {
                name: "generic".into(),
                system_prompt_template: "Clean up the transcript.".into(),
                client_post_process: vec![],
            })
    }
}
