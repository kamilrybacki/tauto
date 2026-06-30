use super::provider::{ArtifactKind, SlmProviderRef};

#[derive(Debug, Clone, PartialEq)]
pub struct ArtifactTraceability {
    pub contract_set_hash: String,
    pub provider: Option<SlmProviderRef>,
    pub target_language: String,
    pub artifact_kind: ArtifactKind,
    pub deterministic_context_hash: String,
}

pub fn build_traceability(
    contract_set_hash: impl Into<String>,
    deterministic_context_hash: impl Into<String>,
    target_language: impl Into<String>,
    artifact_kind: ArtifactKind,
    provider: Option<SlmProviderRef>,
) -> ArtifactTraceability {
    ArtifactTraceability {
        contract_set_hash: contract_set_hash.into(),
        provider,
        target_language: target_language.into(),
        artifact_kind,
        deterministic_context_hash: deterministic_context_hash.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_hash(prefix: &str) -> String {
        format!("{:0<64}", prefix)
    }

    #[test]
    fn traceability_without_provider_is_valid() {
        let t = build_traceability(
            fake_hash("abc"),
            fake_hash("def"),
            "lean4",
            ArtifactKind::ProofAttempt,
            None,
        );
        assert!(t.provider.is_none());
        assert_eq!(t.target_language, "lean4");
    }

    #[test]
    fn traceability_with_provider_stores_ref() {
        let provider = SlmProviderRef {
            name: "deepseek".to_owned(),
            model_id: "deepseek-coder-v2".to_owned(),
        };
        let t = build_traceability(
            fake_hash("abc"),
            fake_hash("def"),
            "lean4",
            ArtifactKind::ProofAttempt,
            Some(provider.clone()),
        );
        assert_eq!(t.provider, Some(provider));
    }

    #[test]
    fn traceability_preserves_hashes() {
        let csh = fake_hash("contract");
        let dch = fake_hash("context");
        let t = build_traceability(
            csh.clone(),
            dch.clone(),
            "lean4",
            ArtifactKind::ProofAttempt,
            None,
        );
        assert_eq!(t.contract_set_hash, csh);
        assert_eq!(t.deterministic_context_hash, dch);
    }

    #[test]
    fn traceability_round_trips_through_build() {
        let t1 = build_traceability(
            fake_hash("x"),
            fake_hash("y"),
            "lean4",
            ArtifactKind::AnnotatedSpec,
            None,
        );
        let t2 = build_traceability(
            fake_hash("x"),
            fake_hash("y"),
            "lean4",
            ArtifactKind::AnnotatedSpec,
            None,
        );
        assert_eq!(t1, t2);
    }
}
