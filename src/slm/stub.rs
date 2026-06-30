/// Deterministic SLM provider stub for testing. Returns a non-empty artifact
/// whose content is derived purely from the request inputs — no network, no model.
use super::provider::{
    CodeGenerationRequest, GeneratedArtifact, SlmCodeGenerator, SlmError,
    SlmProviderRef,
};

pub struct DeterministicStubProvider {
    pub ref_: SlmProviderRef,
}

impl DeterministicStubProvider {
    pub fn new() -> Self {
        Self {
            ref_: SlmProviderRef {
                name: "stub".to_owned(),
                model_id: "deterministic-v0".to_owned(),
            },
        }
    }
}

impl SlmCodeGenerator for DeterministicStubProvider {
    fn generate_code_from_ast(
        &self,
        request: &CodeGenerationRequest,
    ) -> Result<GeneratedArtifact, SlmError> {
        let contract_count = request.contract_set.contracts.len();
        let content = format!(
            "-- Stub output for {} contracts targeting {}\n-- artifact_kind: {:?}\n",
            contract_count, request.target_language, request.artifact_kind
        );
        Ok(GeneratedArtifact {
            content,
            diagnostics: vec![],
            provider: self.ref_.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use crate::contract_ir::{ContractIR, ContractSet};
    use crate::preprocessing::build_deterministic_context;
    use crate::slm::{ArtifactKind, CodeGenerationRequest, build_traceability};

    fn two_contract_set() -> ContractSet {
        ContractSet::new(vec![
            ContractIR::new("CancelPaidOrder", "Order", "cancelOrder"),
            ContractIR::new("ShipOrder", "Order", "shipOrder"),
        ])
    }

    #[test]
    fn stub_produces_non_empty_artifact() {
        let stub = DeterministicStubProvider::new();
        let cs = two_contract_set();
        let request = CodeGenerationRequest {
            contract_set: cs.clone(),
            target_language: "lean4".to_owned(),
            artifact_kind: ArtifactKind::ProofAttempt,
            context: BTreeMap::new(),
        };
        let artifact = stub.generate_code_from_ast(&request).unwrap();
        assert!(!artifact.content.is_empty());
    }

    #[test]
    fn stub_artifact_mentions_contract_count() {
        let stub = DeterministicStubProvider::new();
        let cs = two_contract_set();
        let request = CodeGenerationRequest {
            contract_set: cs,
            target_language: "lean4".to_owned(),
            artifact_kind: ArtifactKind::ProofAttempt,
            context: BTreeMap::new(),
        };
        let artifact = stub.generate_code_from_ast(&request).unwrap();
        assert!(artifact.content.contains("2 contracts"));
    }

    #[test]
    fn stub_provider_ref_matches_declared_ref() {
        let stub = DeterministicStubProvider::new();
        let cs = two_contract_set();
        let request = CodeGenerationRequest {
            contract_set: cs,
            target_language: "lean4".to_owned(),
            artifact_kind: ArtifactKind::ProofAttempt,
            context: BTreeMap::new(),
        };
        let artifact = stub.generate_code_from_ast(&request).unwrap();
        assert_eq!(artifact.provider.name, "stub");
    }

    #[test]
    fn traceability_round_trip_with_stub() {
        let stub = DeterministicStubProvider::new();
        let cs = two_contract_set();
        let ctx = build_deterministic_context(&cs, "lean_gen");

        let request = CodeGenerationRequest {
            contract_set: cs,
            target_language: "lean4".to_owned(),
            artifact_kind: ArtifactKind::ProofAttempt,
            context: ctx.entries.clone(),
        };
        let artifact = stub.generate_code_from_ast(&request).unwrap();

        let traceability = build_traceability(
            ctx.entries["contract_set_hash"].clone(),
            ctx.context_hash.clone(),
            "lean4",
            ArtifactKind::ProofAttempt,
            Some(artifact.provider),
        );

        assert_eq!(traceability.target_language, "lean4");
        assert!(traceability.provider.is_some());
        assert_eq!(traceability.provider.as_ref().unwrap().name, "stub");
        assert_eq!(traceability.contract_set_hash.len(), 64);
    }

    #[test]
    fn stub_is_deterministic_across_calls() {
        let stub = DeterministicStubProvider::new();
        let cs = two_contract_set();
        let request = CodeGenerationRequest {
            contract_set: cs,
            target_language: "lean4".to_owned(),
            artifact_kind: ArtifactKind::ProofAttempt,
            context: BTreeMap::new(),
        };
        let a1 = stub.generate_code_from_ast(&request).unwrap();
        let a2 = stub.generate_code_from_ast(&request).unwrap();
        assert_eq!(a1.content, a2.content);
    }
}
