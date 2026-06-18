use regex::Regex;
use speak_up_core::{AppContext, PostProcessRule, Profile, ProfileMapping};
use std::collections::HashMap;

pub struct ProfileManager {
    profiles: HashMap<String, Profile>,
    rules: Vec<ProfileRule>,
}

struct ProfileRule {
    app_pattern: Regex,
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
                client_post_process: vec![PostProcessRule::TrimWhitespace, PostProcessRule::CapitalizeFirst],
            },
        );
        profiles.insert(
            "email".into(),
            Profile {
                name: "email".into(),
                system_prompt_template: "Format as a professional email. Add appropriate salutation and closing if the transcript suggests one.".into(),
                client_post_process: vec![PostProcessRule::TrimWhitespace, PostProcessRule::CapitalizeFirst],
            },
        );
        profiles.insert(
            "chat".into(),
            Profile {
                name: "chat".into(),
                system_prompt_template: "Keep it brief and conversational. Use natural punctuation.".into(),
                client_post_process: vec![PostProcessRule::TrimWhitespace],
            },
        );
        profiles.insert(
            "code".into(),
            Profile {
                name: "code".into(),
                system_prompt_template: "Preserve code identifiers and syntax exactly as spoken. Add line breaks only where indicated. Do not fix code syntax.".into(),
                client_post_process: vec![PostProcessRule::TrimWhitespace, PostProcessRule::PreserveLineBreaks],
            },
        );
        profiles.insert(
            "terminal".into(),
            Profile {
                name: "terminal".into(),
                system_prompt_template: "Preserve commands and flags exactly. Do not add punctuation or capitalization corrections.".into(),
                client_post_process: vec![PostProcessRule::TrimWhitespace],
            },
        );
        Self { profiles, rules: Vec::new() }
    }

    pub fn load_rules(&mut self, mappings: &[ProfileMapping]) {
        self.rules = mappings
            .iter()
            .filter_map(|m| {
                Regex::new(&m.app_pattern).ok().map(|re| ProfileRule {
                    app_pattern: re,
                    profile_name: m.profile_name.clone(),
                })
            })
            .collect();
        tracing::info!("Loaded {} profile matching rules", self.rules.len());
    }

    pub fn match_profile(&self, context: &AppContext) -> Option<&Profile> {
        for rule in &self.rules {
            let haystack = format!(
                "{} {} {}",
                context.window_title, context.executable_name, context.window_class
            );
            if rule.app_pattern.is_match(&haystack) {
                if let Some(profile) = self.profiles.get(&rule.profile_name) {
                    tracing::debug!(
                        "Matched profile '{}' via pattern '{}'",
                        rule.profile_name,
                        rule.app_pattern.as_str()
                    );
                    return Some(profile);
                }
            }
        }
        None
    }

    pub fn apply_post_process(&self, text: &str, profile: &Profile) -> String {
        let mut result = text.to_string();
        for rule in &profile.client_post_process {
            match rule {
                PostProcessRule::PrefixSpace => {
                    if !result.starts_with(' ') {
                        result.insert(0, ' ');
                    }
                }
                PostProcessRule::TrimWhitespace => {
                    result = result.trim().to_string();
                }
                PostProcessRule::CapitalizeFirst => {
                    if let Some(c) = result.chars().next() {
                        if c.is_lowercase() {
                            let mut chars: Vec<char> = result.chars().collect();
                            chars[0] = c.to_uppercase().next().unwrap_or(c);
                            result = chars.into_iter().collect();
                        }
                    }
                }
                PostProcessRule::PreserveLineBreaks => {}
            }
        }
        result
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
