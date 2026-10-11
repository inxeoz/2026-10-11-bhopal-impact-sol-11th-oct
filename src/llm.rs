use crate::models::Lang;
use serde_json::json;

const BASE: &str = "https://api.cerebras.ai/v1";
const MODEL_REASON: &str = "gpt-oss-120b";
const MODEL_HINDI: &str = "qwen-3.8-27b";

/// Thin Cerebras client. Used only for language: understanding the question
/// and saying the answer in the user's language. Never for numbers.
pub struct Llm {
    client: reqwest::Client,
    key: Option<String>,
}

impl Llm {
    pub fn from_env() -> Self {
        let _ = dotenvy::dotenv();
        let key = std::env::var("CEREBRASAPI").ok().filter(|k| !k.trim().is_empty());
        Llm {
            client: reqwest::Client::new(),
            key,
        }
    }

    /// A client that never calls out — for tests and the offline demo.
    pub fn disabled() -> Self {
        Llm { client: reqwest::Client::new(), key: None }
    }

    pub fn is_available(&self) -> bool {
        self.key.is_some()
    }

    fn model_for(&self, lang: Lang) -> &'static str {
        match lang {
            Lang::Hi => MODEL_HINDI,
            Lang::En => MODEL_REASON,
        }
    }

    /// One chat completion. Returns Err on any transport/parse failure so the
    /// caller can fall back to the deterministic answer.
    async fn chat(
        &self,
        model: &str,
        system: &str,
        user: &str,
        json_mode: bool,
    ) -> Result<String, String> {
        let key = self.key.as_ref().ok_or("no CEREBRASAPI key")?;
        let mut body = json!({
            "model": model,
            "temperature": 0.1,
            // The models reason before answering; a Hindi translation of a long detail took
            // ~1,400 tokens, and a cut-off reply is empty (the English then stays).
            "max_tokens": 3000,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user},
            ],
        });
        if json_mode {
            body["response_format"] = json!({"type": "json_object"});
        }
        let resp = self
            .client
            .post(format!("{BASE}/chat/completions"))
            .bearer_auth(key)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = resp.status();
        let text = resp.text().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            return Err(format!("cerebras {status}: {text}"));
        }
        let v: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("bad json: {e}"))?;
        v["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "no content in response".into())
    }

    /// Rewrite a deterministic answer into clean prose in the target language.
    /// On any failure the caller keeps the code-written text.
    pub async fn polish(&self, lang: Lang, system: &str, user: &str) -> Option<String> {
        if !self.is_available() {
            return None;
        }
        self.chat(self.model_for(lang), system, user, false)
            .await
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    /// Ask the model for a strict JSON object (used for NL parsing only).
    pub async fn json(&self, lang: Lang, system: &str, user: &str) -> Option<serde_json::Value> {
        if !self.is_available() {
            return None;
        }
        let raw = self.chat(self.model_for(lang), system, user, true).await.ok()?;
        serde_json::from_str(raw.trim()).ok()
    }
}
