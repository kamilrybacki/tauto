use crate::contract_ir::Diagnostic;

use super::workspace::LeanWorkspace;

struct ForbiddenPattern {
    token: &'static str,
    category: &'static str,
}

const FORBIDDEN: &[ForbiddenPattern] = &[
    ForbiddenPattern { token: "sorry", category: "lean_sorry" },
    ForbiddenPattern { token: "axiom", category: "lean_axiom" },
    ForbiddenPattern { token: "native_decide", category: "lean_unsafe" },
    ForbiddenPattern { token: "unsafe", category: "lean_unsafe" },
];

pub fn scan_lean_workspace(workspace: &LeanWorkspace) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for file in &workspace.files {
        if !file.path.ends_with(".lean") {
            continue;
        }
        for (line_idx, line) in file.content.lines().enumerate() {
            for pattern in FORBIDDEN {
                if line_contains_token(line, pattern.token) {
                    diagnostics.push(Diagnostic {
                        message: format!(
                            "forbidden token '{}' found in {}:{}",
                            pattern.token,
                            file.path,
                            line_idx + 1
                        ),
                        category: pattern.category.to_owned(),
                        document_path: Some(file.path.clone()),
                        line: Some((line_idx + 1) as u32),
                        suggestion: None,
                    });
                }
            }
        }
    }
    diagnostics
}

fn line_contains_token(line: &str, token: &str) -> bool {
    // Whitespace-bounded match to avoid false positives inside identifiers
    // e.g. "unsafeMethod" should not trigger "unsafe"
    let mut rest = line;
    while let Some(pos) = rest.find(token) {
        let before_ok = pos == 0 || !rest.as_bytes()[pos - 1].is_ascii_alphanumeric();
        let after_pos = pos + token.len();
        let after_ok = after_pos >= rest.len()
            || (!rest.as_bytes()[after_pos].is_ascii_alphanumeric()
                && rest.as_bytes()[after_pos] != b'_');
        if before_ok && after_ok {
            return true;
        }
        rest = &rest[pos + 1..];
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lean_gen::workspace::{LeanWorkspace, LeanWorkspaceFile, generate_lean_workspace};
    use crate::contract_ir::{Condition, ContractIR, ContractSet, Expression, ExpressionValue};

    fn expr(kind: &str, value: &str) -> Expression {
        Expression { kind: kind.to_owned(), value: ExpressionValue::Str(value.to_owned()) }
    }

    fn cond(left: &str, op: &str, right: &str) -> Condition {
        Condition { left: expr("field", left), operator: op.to_owned(), right: expr("enum", right) }
    }

    fn rich_set() -> ContractSet {
        ContractSet::new(vec![ContractIR {
            case: "CancelPaidOrder".to_owned(),
            entity: "Order".to_owned(),
            operation: "cancelOrder".to_owned(),
            requires: vec![cond("order.status", "==", "Paid")],
            ensures: vec![cond("result.status", "==", "Cancelled")],
            forbidden: vec![],
            preserves: vec![],
            assumes: vec![],
            source: None,
        }])
    }

    fn lean_file(path: &str, content: &str) -> LeanWorkspaceFile {
        LeanWorkspaceFile { path: path.to_owned(), content: content.to_owned() }
    }

    #[test]
    fn scan_flags_sorry_in_generated_workspace() {
        // The scanner flags `sorry` wherever it appears. Generated workspaces are
        // now sorry-free (real proofs), so test the scanner on an explicit stub.
        let ws = LeanWorkspace {
            files: vec![lean_file(
                "TautoContracts/contracts/CancelPaidOrder.lean",
                "theorem t : True := by sorry\n",
            )],
        };
        let diags = scan_lean_workspace(&ws);
        assert!(!diags.is_empty());
    }

    #[test]
    fn scan_sorry_diagnostics_have_correct_category() {
        let ws = generate_lean_workspace(&rich_set());
        let diags = scan_lean_workspace(&ws);
        assert!(diags.iter().all(|d| d.category == "lean_sorry"));
    }

    #[test]
    fn scan_reports_file_path_in_diagnostic() {
        let ws = LeanWorkspace {
            files: vec![lean_file(
                "TautoContracts/contracts/CancelPaidOrder.lean",
                "theorem t : True := by sorry\n",
            )],
        };
        let diags = scan_lean_workspace(&ws);
        assert!(diags.iter().all(|d| d.document_path.is_some()));
        assert!(diags
            .iter()
            .any(|d| d.document_path.as_deref().map(|p| p.contains("CancelPaidOrder")).unwrap_or(false)));
    }

    #[test]
    fn scan_reports_positive_line_numbers() {
        let ws = generate_lean_workspace(&rich_set());
        let diags = scan_lean_workspace(&ws);
        assert!(diags.iter().all(|d| d.line.map(|l| l > 0).unwrap_or(false)));
    }

    #[test]
    fn scan_returns_one_diagnostic_per_sorry() {
        let ws = LeanWorkspace {
            files: vec![lean_file(
                "a.lean",
                "theorem a : True := by sorry\ntheorem b : True := by sorry\n",
            )],
        };
        let diags = scan_lean_workspace(&ws);
        assert_eq!(diags.len(), 2, "two sorry stubs → two diagnostics");
    }

    #[test]
    fn scan_clean_file_returns_no_diagnostics() {
        let ws = LeanWorkspace {
            files: vec![lean_file(
                "contracts/Clean.lean",
                "namespace Tauto.Contracts.Clean\n\nend Tauto.Contracts.Clean\n",
            )],
        };
        assert!(scan_lean_workspace(&ws).is_empty());
    }

    #[test]
    fn scan_detects_axiom_token() {
        let ws = LeanWorkspace {
            files: vec![lean_file(
                "contracts/Axiomatic.lean",
                "namespace Tauto\naxiom bad_axiom : True\nend Tauto\n",
            )],
        };
        let diags = scan_lean_workspace(&ws);
        assert!(diags.iter().any(|d| d.category == "lean_axiom"));
    }

    #[test]
    fn scan_detects_native_decide() {
        let ws = LeanWorkspace {
            files: vec![lean_file(
                "contracts/Native.lean",
                "namespace Tauto\nexample : 1 + 1 = 2 := by native_decide\nend Tauto\n",
            )],
        };
        let diags = scan_lean_workspace(&ws);
        assert!(diags.iter().any(|d| d.category == "lean_unsafe"));
    }

    #[test]
    fn scan_detects_unsafe_keyword() {
        let ws = LeanWorkspace {
            files: vec![lean_file(
                "contracts/Unsafe.lean",
                "namespace Tauto\nunsafe def bad : Nat := 0\nend Tauto\n",
            )],
        };
        let diags = scan_lean_workspace(&ws);
        assert!(diags.iter().any(|d| d.category == "lean_unsafe"));
    }

    #[test]
    fn scan_skips_non_lean_files() {
        let ws = LeanWorkspace {
            files: vec![lean_file("lakefile.toml", "# config\nsorry = 'value'\n")],
        };
        assert!(scan_lean_workspace(&ws).is_empty());
    }

    #[test]
    fn scan_does_not_trigger_on_identifier_containing_token() {
        // "unsafeMethod" must NOT trigger "unsafe" token
        let ws = LeanWorkspace {
            files: vec![lean_file(
                "contracts/Safe.lean",
                "namespace Tauto\ndef unsafeMethod : Nat := 0\nend Tauto\n",
            )],
        };
        let diags = scan_lean_workspace(&ws);
        assert!(
            diags.iter().all(|d| d.category != "lean_unsafe"),
            "token inside identifier must not trigger"
        );
    }
}
