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

/// Select the prose→DSL translator from the environment.
///
/// SLM translation is a REQUIRED capability, so a real provider (DeepSeek) is the
/// default and this fails loudly if it is not configured — it never silently
/// falls back to the stub. The deterministic stub is used only when explicitly
/// requested with `TAUTO_SLM_PROVIDER=stub` (offline/testing).
pub fn translator_from_env() -> Result<Box<dyn SlmTranslator>, SlmError> {
    match std::env::var("TAUTO_SLM_PROVIDER").ok().as_deref() {
        Some("stub") => Ok(Box::new(DeterministicStubProvider::new())),
        _ => DeepSeekProvider::from_env().map(|p| Box::new(p) as Box<dyn SlmTranslator>),
    }
}
