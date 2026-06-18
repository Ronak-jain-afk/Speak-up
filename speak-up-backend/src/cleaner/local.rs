use super::*;

const FILLER_WORDS: &[&str] = &[
    "um", "uh", "er", "ah", "like", "you know", "sort of", "kind of",
    "actually", "basically", "literally", "I mean", "you see", "well",
];

pub struct LocalLLM;

impl LocalLLM {
    pub fn new() -> Self {
        Self
    }
}

impl AICleaner for LocalLLM {
    fn clean(
        &self,
        transcript: &str,
        _profile: &Profile,
        dictionary: &[DictionaryEntry],
    ) -> Result<String, CleanerError> {
        let mut text = transcript.to_string();

        text = remove_filler_words(&text);
        text = apply_dictionary(&text, dictionary);
        text = restore_punctuation(&text);
        text = capitalize_sentences(&text);

        Ok(text.trim().to_string())
    }

    fn shutdown(&mut self) {}
}

fn remove_filler_words(text: &str) -> String {
    let mut result = text.to_string();
    for word in FILLER_WORDS {
        let pattern = format!(r"(?i)\b{}\b,?\s*", regex::escape(word));
        if let Ok(re) = regex::Regex::new(&pattern) {
            result = re.replace_all(&result, "").to_string();
        }
    }
    result
}

fn apply_dictionary(text: &str, dictionary: &[DictionaryEntry]) -> String {
    let mut result = text.to_string();
    for entry in dictionary {
        let pattern = format!(r"(?i)\b{}\b", regex::escape(&entry.spoken_form));
        if let Ok(re) = regex::Regex::new(&pattern) {
            result = re.replace_all(&result, entry.written_form.as_str()).to_string();
        }
    }
    result
}

fn restore_punctuation(text: &str) -> String {
    let mut result = text.to_string();

    if !result.ends_with('.') && !result.ends_with('!') && !result.ends_with('?') {
        let trimmed = result.trim_end();
        if !trimmed.is_empty() {
            let last = trimmed.chars().last().unwrap();
            if last.is_alphabetic() || last == '"' || last == '\'' || last == ')' {
                result = format!("{}.", trimmed);
            }
        }
    }

    result
}

fn capitalize_sentences(text: &str) -> String {
    let mut result = String::new();
    let mut new_sentence = true;

    for c in text.chars() {
        if new_sentence && c.is_alphabetic() {
            for upper in c.to_uppercase() {
                result.push(upper);
            }
            new_sentence = false;
        } else {
            if c == '.' || c == '!' || c == '?' {
                new_sentence = true;
            } else if !c.is_whitespace() {
                new_sentence = false;
            }
            result.push(c);
        }
    }

    result
}

impl Default for LocalLLM {
    fn default() -> Self {
        Self::new()
    }
}
