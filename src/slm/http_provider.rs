use reqwest::blocking::Client;
use serde_json::json;

use super::provider::{
    CodeGenerationRequest, GeneratedArtifact, SlmCodeGenerator, SlmError, SlmProviderRef,
};

pub struct DeepSeekProvider {
    api_key: String,
    model_id: String,
    client: Client,
}

impl std::fmt::Debug for DeepSeekProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeepSeekProvider")
            .field("model_id", &self.model_id)
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
        Self { api_key: api_key.into(), model_id, client: Client::new() }
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

        let body = json!({
            "model": self.model_id,
            "messages": [{ "role": "user", "content": prompt }],
            "temperature": 0,
            "max_tokens": 2048,
        });

        let response = self
            .client
            .post("https://api.deepseek.com/v1/chat/completions")
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .map_err(|e| SlmError::ProviderError(format!("request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(SlmError::ProviderError(format!("API error {status}: {text}")));
        }

        let parsed: serde_json::Value = response
            .json()
            .map_err(|e| SlmError::ProviderError(format!("response parse error: {e}")))?;

        let content = parsed["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| SlmError::ProviderError("no content in response".to_owned()))?
            .to_owned();

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
        let prompt = super::translate::build_translation_prompt(request);
        let body = json!({
            "model": self.model_id,
            "messages": [{ "role": "user", "content": prompt }],
            "temperature": 0,
            // DSL blocks are short; cap output to keep each translation cheap.
            "max_tokens": 1024,
        });
        let response = self
            .client
            .post("https://api.deepseek.com/v1/chat/completions")
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .map_err(|e| SlmError::ProviderError(format!("request failed: {e}")))?;
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(SlmError::ProviderError(format!("API error {status}: {text}")));
        }
        let parsed: serde_json::Value = response
            .json()
            .map_err(|e| SlmError::ProviderError(format!("response parse error: {e}")))?;
        let content = parsed["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| SlmError::ProviderError("no content in response".to_owned()))?;
        let dsl = super::translate::extract_dsl(content);
        Ok(super::translate::TranslationResult {
            dsl,
            notes: vec![],
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
}
