use serde::{Deserialize, Serialize};

use crate::contract_ir::{ContractSet, Diagnostic};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtifactKind {
    ProofAttempt,
    AnnotatedSpec,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlmProviderRef {
    pub name: String,
    pub model_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CodeGenerationRequest {
    pub contract_set: ContractSet,
    pub target_language: String,
    pub artifact_kind: ArtifactKind,
    pub context: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeneratedArtifact {
    pub content: String,
    pub diagnostics: Vec<Diagnostic>,
    pub provider: SlmProviderRef,
}

pub trait SlmCodeGenerator {
    fn generate_code_from_ast(
        &self,
        request: &CodeGenerationRequest,
    ) -> Result<GeneratedArtifact, SlmError>;
}

#[derive(Debug, thiserror::Error)]
pub enum SlmError {
    #[error("provider error: {0}")]
    ProviderError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slm_provider_ref_fields_accessible() {
        let r = SlmProviderRef {
            name: "deepseek".to_owned(),
            model_id: "deepseek-coder-v2".to_owned(),
        };
        assert_eq!(r.name, "deepseek");
        assert_eq!(r.model_id, "deepseek-coder-v2");
    }

    #[test]
    fn artifact_kind_variants_exist() {
        let _ = ArtifactKind::ProofAttempt;
        let _ = ArtifactKind::AnnotatedSpec;
    }
}
