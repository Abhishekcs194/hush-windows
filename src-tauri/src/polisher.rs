use serde::{Deserialize, Serialize};

use crate::context::AppCategory;
use crate::history::HistoryEntry;
use crate::transcriber::TranscriptionSegment;

const GROQ_URL: &str = "https://api.groq.com/openai/v1/chat/completions";
const GROQ_MODEL: &str = "llama-3.1-8b-instant";
const TIMEOUT_SECS: u64 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum CleanupLevel {
    Off,
    Light,
    Standard,
    Polished,
}

impl CleanupLevel {
    pub fn is_on(&self) -> bool {
        !matches!(self, Self::Off)
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Light => "Light",
            Self::Standard => "Standard",
            Self::Polished => "Polished",
        }
    }

    pub fn max_output_ratio(&self) -> f32 {
        match self {
            Self::Off => 1.0,
            Self::Light => 1.0,
            Self::Standard | Self::Polished => 3.0,
        }
    }

    fn instruction(&self) -> &'static str {
        match self {
            Self::Off => "",
            Self::Light => "Fix obvious transcription errors and punctuation only. Do not rephrase.",
            Self::Standard => "Clean up grammar and punctuation. Fix homophones. Keep the speaker's voice.",
            Self::Polished => "Rewrite for clarity and flow. Remove filler words. Keep meaning intact.",
        }
    }
}

pub struct Polisher {
    client: reqwest::Client,
    token: Option<String>,
}

impl Polisher {
    pub fn new(token: Option<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
            .build()
            .expect("Failed to build HTTP client");
        Self { client, token }
    }

    pub fn set_token(&mut self, token: Option<String>) {
        self.token = token;
    }

    pub async fn polish(
        &self,
        segments: &[TranscriptionSegment],
        level: CleanupLevel,
        category: AppCategory,
        user_profile: Option<&str>,
        recent_context: &[HistoryEntry],
    ) -> String {
        let raw = merge_segments(segments);
        if raw.is_empty() {
            return raw;
        }

        let token = match &self.token {
            Some(t) => t.clone(),
            None => return raw,
        };

        if !level.is_on() {
            return raw;
        }

        match self
            .call_groq(&raw, level, category, user_profile, recent_context, &token)
            .await
        {
            Ok(polished) => {
                if is_output_valid(&raw, &polished, level) {
                    polished
                } else {
                    log::warn!("Polish output rejected (length guard), using raw");
                    raw
                }
            }
            Err(e) => {
                log::warn!("Polish failed: {}, using raw text", e);
                raw
            }
        }
    }

    async fn call_groq(
        &self,
        raw: &str,
        level: CleanupLevel,
        category: AppCategory,
        user_profile: Option<&str>,
        recent_context: &[HistoryEntry],
        token: &str,
    ) -> anyhow::Result<String> {
        let system = build_system_prompt(level, category, user_profile, recent_context);
        let max_tokens = (raw.split_whitespace().count() as u32 * 2).max(64);

        let req = GroqRequest {
            model: GROQ_MODEL.to_string(),
            messages: vec![
                Message { role: "system".into(), content: system },
                Message { role: "user".into(), content: raw.to_string() },
            ],
            max_tokens,
            temperature: 0.0,
            response_format: ResponseFormat { format_type: "json_object".into() },
        };

        let resp = self
            .client
            .post(GROQ_URL)
            .bearer_auth(token)
            .json(&req)
            .send()
            .await?
            .error_for_status()?;

        let groq_resp: GroqResponse = resp.json().await?;
        let content = groq_resp
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Empty Groq response"))?
            .message
            .content;

        let parsed: PolishedPayload = serde_json::from_str(&content)?;
        Ok(parsed.text.trim().to_string())
    }
}

fn merge_segments(segments: &[TranscriptionSegment]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (i, seg) in segments.iter().enumerate() {
        if i > 0 {
            let pause = seg.pause_before_secs;
            if pause >= 2.0 {
                parts.push("<break>".to_string());
            } else if pause >= 0.5 {
                parts.push(format!("<pause {:.1}s>", pause));
            }
        }
        parts.push(seg.text.clone());
    }
    parts.join(" ")
}

fn build_system_prompt(
    level: CleanupLevel,
    category: AppCategory,
    user_profile: Option<&str>,
    recent_context: &[HistoryEntry],
) -> String {
    let mut prompt = format!(
        "You clean up voice dictation transcriptions. Respond ONLY with JSON: {{\"text\": \"...\"}}\n\n\
         Edit level: {}\n\
         App context: {}\n\
         {}",
        level.instruction(),
        category.format_hint(),
        category.format_hint(),
    );

    if let Some(profile) = user_profile {
        if !profile.is_empty() {
            prompt.push_str(&format!("\n\nUser profile: {}", profile));
        }
    }

    if !recent_context.is_empty() {
        prompt.push_str("\n\nRecent dictations in this app (for continuity):");
        for entry in recent_context.iter().take(3) {
            prompt.push_str(&format!("\n- {}", entry.text));
        }
    }

    prompt.push_str(
        "\n\nRules:\n\
         - <pause Xs> markers indicate speaker pauses — use them to decide sentence boundaries\n\
         - <break> markers are hard paragraph breaks\n\
         - Keep output close to the input length\n\
         - Do not add information not present in the input",
    );

    prompt
}

fn is_output_valid(raw: &str, output: &str, level: CleanupLevel) -> bool {
    let raw_words = raw.split_whitespace().count();
    let out_words = output.split_whitespace().count();
    if raw_words == 0 {
        return !output.is_empty();
    }
    let ratio = out_words as f32 / raw_words as f32;
    ratio <= level.max_output_ratio()
}

#[derive(Serialize)]
struct GroqRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: u32,
    temperature: f32,
    response_format: ResponseFormat,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    format_type: String,
}

#[derive(Deserialize)]
struct GroqResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: MessageContent,
}

#[derive(Deserialize)]
struct MessageContent {
    content: String,
}

#[derive(Deserialize)]
struct PolishedPayload {
    text: String,
}
