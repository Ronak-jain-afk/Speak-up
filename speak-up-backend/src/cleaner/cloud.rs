use super::*;

pub struct OpenAICleaner {
    api_key: String,
    client: reqwest::blocking::Client,
}

impl OpenAICleaner {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }

    fn build_prompt(&self, transcript: &str, profile: &Profile, dictionary: &[DictionaryEntry]) -> String {
        let mut prompt = String::from(
            "You are a transcript cleaner. Clean up this spoken transcript.\n\nRules:\n"
        );
        prompt.push_str("- Add proper punctuation and capitalization\n");
        prompt.push_str("- Remove filler words: um, uh, like, you know, sort of, kind of, actually, basically, literally, I mean\n");
        prompt.push_str("- Fix grammar while keeping the original meaning\n");
        prompt.push_str("- Do not add information not in the original\n");
        prompt.push_str("- Preserve technical terms and code exactly\n\n");

        if !profile.system_prompt_template.is_empty() && profile.name != "generic" {
            prompt.push_str(&format!("Profile instructions: {}\n\n", profile.system_prompt_template));
        }

        if !dictionary.is_empty() {
            prompt.push_str("Dictionary rules:\n");
            for entry in dictionary {
                prompt.push_str(&format!(
                    "- Write '{}' when you hear '{}'\n",
                    entry.written_form, entry.spoken_form
                ));
            }
            prompt.push('\n');
        }

        prompt.push_str("Transcript:\n");
        prompt.push_str(transcript);
        prompt
    }
}

impl AICleaner for OpenAICleaner {
    fn clean(
        &self,
        transcript: &str,
        profile: &Profile,
        dictionary: &[DictionaryEntry],
    ) -> Result<String, CleanerError> {
        if self.api_key.is_empty() {
            return Err(CleanerError::Auth("OpenAI API key not configured".into()));
        }

        let prompt = self.build_prompt(transcript, profile, dictionary);

        let body = serde_json::json!({
            "model": "gpt-4o-mini",
            "messages": [
                {"role": "user", "content": prompt}
            ],
            "temperature": 0.2,
            "max_tokens": 4096
        });

        let response = self
            .client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .map_err(|e| CleanerError::Http(format!("Request failed: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let body_text = response.text().unwrap_or_default();
            if status.as_u16() == 401 {
                return Err(CleanerError::Auth(format!("Invalid API key: {}", body_text)));
            }
            return Err(CleanerError::Http(format!("API error ({}): {}", status, body_text)));
        }

        let json: serde_json::Value = response
            .json()
            .map_err(|e| CleanerError::Http(format!("Parse failed: {}", e)))?;

        let cleaned = json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| CleanerError::Inference("No content in response".into()))?;

        Ok(cleaned.trim().to_string())
    }

    fn shutdown(&mut self) {}
}

pub struct AnthropicCleaner {
    api_key: String,
    client: reqwest::blocking::Client,
}

impl AnthropicCleaner {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }

    fn build_prompt(&self, transcript: &str, profile: &Profile, dictionary: &[DictionaryEntry]) -> String {
        let mut prompt = String::from("Clean up this spoken transcript.\n\nRules:\n");
        prompt.push_str("- Add proper punctuation and capitalization\n");
        prompt.push_str("- Remove filler words: um, uh, like, you know, sort of, kind of, actually, basically, literally, I mean\n");
        prompt.push_str("- Fix grammar while keeping the original meaning\n");
        prompt.push_str("- Do not add information not in the original\n");
        prompt.push_str("- Preserve technical terms and code exactly\n\n");

        if !profile.system_prompt_template.is_empty() && profile.name != "generic" {
            prompt.push_str(&format!("Profile instructions: {}\n\n", profile.system_prompt_template));
        }

        if !dictionary.is_empty() {
            prompt.push_str("Dictionary rules:\n");
            for entry in dictionary {
                prompt.push_str(&format!(
                    "- Write '{}' when you hear '{}'\n",
                    entry.written_form, entry.spoken_form
                ));
            }
            prompt.push('\n');
        }

        prompt.push_str("Transcript:\n");
        prompt.push_str(transcript);
        prompt
    }
}

impl AICleaner for AnthropicCleaner {
    fn clean(
        &self,
        transcript: &str,
        profile: &Profile,
        dictionary: &[DictionaryEntry],
    ) -> Result<String, CleanerError> {
        if self.api_key.is_empty() {
            return Err(CleanerError::Auth("Anthropic API key not configured".into()));
        }

        let prompt = self.build_prompt(transcript, profile, dictionary);

        let body = serde_json::json!({
            "model": "claude-sonnet-4-20250514",
            "max_tokens": 4096,
            "messages": [
                {"role": "user", "content": prompt}
            ]
        });

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .map_err(|e| CleanerError::Http(format!("Request failed: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let body_text = response.text().unwrap_or_default();
            if status.as_u16() == 401 {
                return Err(CleanerError::Auth(format!("Invalid API key: {}", body_text)));
            }
            return Err(CleanerError::Http(format!("API error ({}): {}", status, body_text)));
        }

        let json: serde_json::Value = response
            .json()
            .map_err(|e| CleanerError::Http(format!("Parse failed: {}", e)))?;

        let cleaned = json["content"][0]["text"]
            .as_str()
            .ok_or_else(|| CleanerError::Inference("No content in response".into()))?;

        Ok(cleaned.trim().to_string())
    }

    fn shutdown(&mut self) {}
}
