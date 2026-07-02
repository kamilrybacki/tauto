pub mod http_provider;
pub mod provider;
pub mod stub;
pub mod traceability;
pub mod translate;

pub use http_provider::DeepSeekProvider;
pub use provider::{
    ArtifactKind, CodeGenerationRequest, GeneratedArtifact, SlmCodeGenerator, SlmError,
    SlmProviderRef,
};
pub use stub::DeterministicStubProvider;
pub use traceability::{ArtifactTraceability, build_traceability};
pub use translate::{SlmTranslator, TranslationRequest, TranslationResult};

/// Select the prose→DSL translator from the environment. Defaults to the
/// deterministic stub (no network); a live DeepSeek translator is used ONLY when
/// `TAUTO_SLM_PROVIDER=deepseek` and `DEEPSEEK_API_KEY` are both set. This keeps
/// live model calls strictly opt-in.
pub fn translator_from_env() -> Box<dyn SlmTranslator> {
    if std::env::var("TAUTO_SLM_PROVIDER").as_deref() == Ok("deepseek") {
        if let Ok(p) = DeepSeekProvider::from_env() {
            return Box::new(p);
        }
    }
    Box::new(DeterministicStubProvider::new())
}
