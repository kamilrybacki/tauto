//! Prose → DSL translation: the SLM front door to the verified pipeline.
//!
//! An SLM turns a natural-language business rule into tauto's **DSL** (not Lean
//! directly). The DSL is legible, so a human/agent reviews it before anything is
//! proven — that review is the faithfulness checkpoint. Downstream, the trusted
//! deterministic DSL → IR → Lean → lake path runs unchanged.
//!
//! Translation NEVER persists and never proves on its own; callers review the
//! returned DSL and then POST it to `/api/v1/check`.

use serde::{Deserialize, Serialize};

use super::provider::{SlmError, SlmProviderRef};

/// A request to translate prose into the contract DSL.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranslationRequest {
    /// The natural-language business rule(s).
    pub prose: String,
    /// Optional hints (e.g. glossary vocabulary) the provider may use.
    #[serde(default)]
    pub context: std::collections::BTreeMap<String, String>,
}

/// The translation result: DSL markdown (one or more ```contract blocks) plus
/// advisory notes. The caller reviews the DSL before checking it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranslationResult {
    pub dsl: String,
    #[serde(default)]
    pub notes: Vec<String>,
    pub provider: SlmProviderRef,
}

/// A provider that translates prose into the contract DSL. `Send + Sync` so a
/// single instance (and its pooled HTTP client) can be built once and shared
/// across requests via server state.
pub trait SlmTranslator: Send + Sync {
    fn translate(&self, request: &TranslationRequest) -> Result<TranslationResult, SlmError>;
}

/// The DSL grammar an SLM must target — shared by prompt construction and docs.
pub const DSL_GUIDE: &str = "\
tauto contract DSL. Emit one or more fenced ```contract blocks. Each block:
  case <PascalCaseName>
  entity:
    <EntityName>
  operation:
    <operationName>
  requires:
    <field.path> <op> <value>      # preconditions
  ensures:
    <field.path> <op> <value>      # postconditions on result.*
  forbidden:
    <operation>(<args>)            # operations that must not occur
  preserves:
    <field.path>                   # fields left unchanged
  assumes:
    <free-form fact>
Operators: == != >= <= > <. Values: integers, true/false, or Uppercase enum
members (e.g. Approved). Field paths are lowercase dotted (loan.credit_score);
postconditions use the result.* prefix. Guard on the entity's state field.";

/// Build the prose→DSL prompt for a chat SLM. Public so it is unit-testable
/// without any network call.
pub fn build_translation_prompt(req: &TranslationRequest) -> String {
    let glossary = req
        .context
        .get("glossary")
        .map(|g| format!("\n\nDomain vocabulary (use these exact entity/field/enum names):\n{g}"))
        .unwrap_or_default();
    format!(
        "You translate business rules into a formal DSL. Output ONLY the DSL — \
fenced ```contract blocks, no prose, no explanation.\n\n{DSL_GUIDE}{glossary}\n\n\
Business rule(s) to translate:\n{}\n",
        req.prose
    )
}

/// Extract the ```contract blocks from a model response. If the model returned
/// exactly the blocks (or wrapped them in prose), keep only the fenced blocks;
/// if it returned bare DSL with no fences, pass it through unchanged.
pub fn extract_dsl(content: &str) -> String {
    let mut blocks = Vec::new();
    let mut in_block = false;
    let mut cur: Vec<&str> = Vec::new();
    for line in content.lines() {
        let t = line.trim();
        if !in_block && t == "```contract" {
            in_block = true;
            cur.clear();
            continue;
        }
        if in_block && t == "```" {
            blocks.push(format!("```contract\n{}\n```", cur.join("\n")));
            in_block = false;
            continue;
        }
        if in_block {
            cur.push(line);
        }
    }
    if blocks.is_empty() {
        content.trim().to_owned()
    } else {
        blocks.join("\n\n")
    }
}

/// Like [`extract_dsl`], but also returns advisory notes. If the extracted text
/// has no recognizable `case ` line, the model likely returned something other
/// than the DSL (e.g. stray prose or Lean) — flag it so the caller reviews,
/// upholding the prose→DSL-only boundary.
pub fn extract_dsl_checked(content: &str) -> (String, Vec<String>) {
    let dsl = extract_dsl(content);
    let mut notes = Vec::new();
    if !dsl.contains("case ") {
        notes.push(
            "SLM output contained no recognizable `case` block — review carefully; it may not \
             be valid contract DSL."
                .to_owned(),
        );
    }
    (dsl, notes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(prose: &str) -> TranslationRequest {
        TranslationRequest { prose: prose.to_owned(), context: Default::default() }
    }

    #[test]
    fn prompt_embeds_grammar_and_prose() {
        let p = build_translation_prompt(&req("An order can ship only if paid."));
        assert!(p.contains("```contract"));
        assert!(p.contains("Operators: == != >="));
        assert!(p.contains("An order can ship only if paid."));
    }

    #[test]
    fn prompt_includes_glossary_when_present() {
        let mut r = req("x");
        r.context.insert("glossary".to_owned(), "entity Order: status".to_owned());
        let p = build_translation_prompt(&r);
        assert!(p.contains("Domain vocabulary"));
        assert!(p.contains("entity Order: status"));
    }

    #[test]
    fn extract_dsl_keeps_only_contract_blocks() {
        let resp = "Here you go:\n```contract\ncase A\nentity:\n  Order\n```\nHope that helps!";
        let dsl = extract_dsl(resp);
        assert!(dsl.starts_with("```contract"));
        assert!(dsl.contains("case A"));
        assert!(!dsl.contains("Hope that helps"));
    }

    #[test]
    fn extract_dsl_passes_through_bare_dsl() {
        let resp = "case A\nentity:\n  Order";
        assert_eq!(extract_dsl(resp), "case A\nentity:\n  Order");
    }

    #[test]
    fn extract_dsl_checked_flags_non_dsl_output() {
        let (_dsl, notes) = extract_dsl_checked("theorem x : True := by trivial");
        assert!(notes.iter().any(|n| n.contains("no recognizable `case`")));
        let (_dsl, notes) = extract_dsl_checked("```contract\ncase A\nentity:\n  Order\n```");
        assert!(notes.is_empty());
    }

    #[test]
    fn stub_translator_scaffolds_and_flags() {
        use crate::slm::{DeterministicStubProvider, SlmTranslator};
        let out = DeterministicStubProvider::new()
            .translate(&req("An order ships only when paid."))
            .unwrap();
        assert!(out.dsl.contains("```contract"));
        assert!(out.dsl.contains("An order ships only when paid."));
        assert_eq!(out.provider.name, "stub");
        assert!(out.notes.iter().any(|n| n.contains("no real translation")));
    }
}
