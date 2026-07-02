use reqwest::blocking::Client;
use serde_json::json;

use super::provider::{
    CodeGenerationRequest, GeneratedArtifact, SlmCodeGenerator, SlmError, SlmProviderRef,
};

/// An OpenAI-compatible chat provider. Defaults to DeepSeek, but the base URL
/// and model are configurable, so it can point at ANY OpenAI-compatible endpoint
/// — a hosted API or a local Ollama / vLLM / llama.cpp server — the same
/// "any deployment" pattern as the lake worker.
pub struct DeepSeekProvider {
    api_key: String,
    model_id: String,
    base_url: String,
    client: Client,
}

impl std::fmt::Debug for DeepSeekProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeepSeekProvider")
            .field("model_id", &self.model_id)
            .field("base_url", &self.base_url)
            .field("api_key", &"[redacted]")
            .finish()
    }
}

impl DeepSeekProvider {
    pub fn from_env() -> Result<Self, SlmError> {
        let api_key = std::env::var("DEEPSEEK_API_KEY").map_err(|_| {
            SlmError::ProviderError("DEEPSEEK_API_KEY env var not set".to_owned())
        })?;
        Ok(Self::new_with_key(api_key))
    }

    pub fn new_with_key(api_key: impl Into<String>) -> Self {
        // Default to the cheapest DeepSeek model — `deepseek-chat` (V3) is far
        // cheaper than `deepseek-reasoner` (R1), and prose→DSL translation needs
        // no reasoning tier. Overridable via DEEPSEEK_MODEL.
        let model_id =
            std::env::var("DEEPSEEK_MODEL").unwrap_or_else(|_| "deepseek-chat".to_owned());
        // Base URL: any OpenAI-compatible endpoint. SLM_BASE_URL (generic) wins,
        // then DEEPSEEK_BASE_URL, else the DeepSeek default. Point at a local
        // Ollama/vLLM (e.g. http://host:11434) to self-host.
        let base_url = std::env::var("SLM_BASE_URL")
            .or_else(|_| std::env::var("DEEPSEEK_BASE_URL"))
            .unwrap_or_else(|_| "https://api.deepseek.com".to_owned());
        // A finite timeout so a stalled endpoint surfaces as a provider error
        // (→ 503) instead of hanging the request. Generous enough for slow local
        // CPU inference.
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { api_key: api_key.into(), model_id, base_url, client }
    }

    /// The chat-completions endpoint for the configured base URL.
    fn chat_url(&self) -> String {
        format!("{}/v1/chat/completions", self.base_url.trim_end_matches('/'))
    }

    /// POST a single-message chat completion and return the assistant text.
    /// Retries only *transient* failures — connection/timeout errors, 5xx, and
    /// 429 — up to 3 attempts with short linear backoff. Never retries 4xx
    /// (e.g. 402 Insufficient Balance, 401 bad key): those are permanent and
    /// billable, so a retry would waste money without helping.
    fn chat_completion(&self, prompt: &str, max_tokens: u32) -> Result<String, SlmError> {
        let body = json!({
            "model": self.model_id,
            "messages": [{ "role": "user", "content": prompt }],
            "temperature": 0,
            "max_tokens": max_tokens,
        });
        let mut attempt = 0u32;
        let parsed: serde_json::Value = loop {
            attempt += 1;
            match self.client.post(self.chat_url()).bearer_auth(&self.api_key).json(&body).send() {
                Ok(r) if r.status().is_success() => {
                    break r.json().map_err(|e| {
                        SlmError::ProviderError(format!("response parse error: {e}"))
                    })?;
                }
                Ok(r) => {
                    let status = r.status();
                    let transient = status.is_server_error() || status.as_u16() == 429;
                    if transient && attempt < 3 {
                        std::thread::sleep(std::time::Duration::from_millis(300 * attempt as u64));
                        continue;
                    }
                    let text = r.text().unwrap_or_default();
                    return Err(SlmError::ProviderError(format!("API error {status}: {text}")));
                }
                Err(e) => {
                    if (e.is_timeout() || e.is_connect()) && attempt < 3 {
                        std::thread::sleep(std::time::Duration::from_millis(300 * attempt as u64));
                        continue;
                    }
                    return Err(SlmError::ProviderError(format!("request failed: {e}")));
                }
            }
        };
        parsed["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| SlmError::ProviderError("no content in response".to_owned()))
            .map(|s| s.to_owned())
    }

    fn build_prompt(lean_stub: &str) -> String {
        format!(
            "You are a Lean 4 proof assistant. Replace each `sorry` in the following Lean 4 \
file with a valid proof. The theorems all have conclusion `True`, so replace `sorry` with \
`trivial`.\nReturn ONLY the complete Lean 4 file with the replacements. No markdown, no \
explanation.\n\n{lean_stub}"
        )
    }
}

impl SlmCodeGenerator for DeepSeekProvider {
    fn generate_code_from_ast(
        &self,
        request: &CodeGenerationRequest,
    ) -> Result<GeneratedArtifact, SlmError> {
        let lean_stub = request.context.get("lean_stub").cloned().unwrap_or_default();
        let prompt = Self::build_prompt(&lean_stub);
        let content = self.chat_completion(&prompt, 2048)?;

        Ok(GeneratedArtifact {
            content,
            diagnostics: vec![],
            provider: SlmProviderRef {
                name: "deepseek".to_owned(),
                model_id: self.model_id.clone(),
            },
        })
    }
}

impl super::translate::SlmTranslator for DeepSeekProvider {
    /// Translate prose → DSL. NOTE: this performs a live API call; it is only
    /// reached when the caller has explicitly selected the DeepSeek provider
    /// (config + key). Tests never invoke this — they test the stub and the
    /// pure prompt/parse helpers.
    fn translate(
        &self,
        request: &super::translate::TranslationRequest,
    ) -> Result<super::translate::TranslationResult, SlmError> {
        // DSL blocks are short; cap output at 1024 to keep each translation cheap.
        let prompt = super::translate::build_translation_prompt(request);
        let content = self.chat_completion(&prompt, 1024)?;
        let (dsl, notes) = super::translate::extract_dsl_checked(&content);
        Ok(super::translate::TranslationResult {
            dsl,
            notes,
            provider: SlmProviderRef { name: "deepseek".to_owned(), model_id: self.model_id.clone() },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lean_stub() -> &'static str {
        "namespace Tauto.Contracts.Test\n\ntheorem op_ensures :\n  True := by\n  sorry\n\nend Tauto.Contracts.Test\n"
    }

    #[test]
    fn from_env_errors_when_key_missing() {
        // Unset the key for this test (safe: other tests don't rely on this var being set)
        std::env::remove_var("DEEPSEEK_API_KEY");
        let result = DeepSeekProvider::from_env();
        assert!(matches!(result, Err(SlmError::ProviderError(_))));
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("DEEPSEEK_API_KEY"));
    }

    #[test]
    fn build_prompt_contains_lean_stub() {
        let prompt = DeepSeekProvider::build_prompt(lean_stub());
        assert!(prompt.contains("sorry"), "prompt must include the sorry stub");
        assert!(prompt.contains("namespace Tauto.Contracts.Test"));
    }

    #[test]
    fn build_prompt_instructs_trivial_replacement() {
        let prompt = DeepSeekProvider::build_prompt(lean_stub());
        assert!(prompt.contains("trivial"), "prompt must instruct to use trivial");
    }

    #[test]
    fn new_with_key_uses_deepseek_chat_model() {
        let p = DeepSeekProvider::new_with_key("test-key");
        assert_eq!(p.model_id, "deepseek-chat");
    }

    #[test]
    fn chat_url_defaults_to_deepseek_and_appends_path() {
        let p = DeepSeekProvider::new_with_key("k");
        // Default (no SLM_BASE_URL/DEEPSEEK_BASE_URL in this test env).
        assert_eq!(p.chat_url(), "https://api.deepseek.com/v1/chat/completions");
    }

    #[test]
    fn chat_url_strips_trailing_slash() {
        let mut p = DeepSeekProvider::new_with_key("k");
        p.base_url = "http://ollama:11434/".to_owned();
        assert_eq!(p.chat_url(), "http://ollama:11434/v1/chat/completions");
    }
}
