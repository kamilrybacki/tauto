use std::path::PathBuf;

use clap::Parser;

use tauto::contract_ir::{
    compare, contract_set_hash, find_conflict_candidates, semantic_contract_set_hash, Condition,
    ContractSet,
};
use tauto::contract_parser::{extract_contract_blocks, parse_contract_block};
use tauto::lean_gen::{
    LakeError, LeanWorkspace, check_lean_available, generate_lean_workspace, run_lake_build,
    scan_lean_workspace, write_lean_workspace,
};
use tauto::project_store::{save_document, ContractDocument};
use tauto::slm::{ArtifactKind, CodeGenerationRequest, DeepSeekProvider, SlmCodeGenerator};

#[derive(clap::ValueEnum, Clone, Debug, PartialEq)]
enum OutputFormat {
    Text,
    Json,
}

#[derive(Parser)]
#[command(name = "tauto", about = "Lean-backed business contract verifier")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Parse contracts and generate a Lean 4 workspace
    Verify {
        /// Directory or file containing contract markdown (recursive)
        path: PathBuf,
        /// Where to write the generated Lean workspace
        #[arg(long, default_value = "lean-workspace")]
        output: PathBuf,
        /// Exit with code 1 if any sorry stubs remain (for CI)
        #[arg(long)]
        strict: bool,
        /// Output format
        #[arg(long, default_value = "text", value_enum)]
        format: OutputFormat,
        /// Attempt SLM proof generation for sorry stubs (e.g. "deepseek")
        #[arg(long)]
        model: Option<String>,
        /// Run `lake build` on the generated workspace to validate Lean 4 syntax
        #[arg(long)]
        lean_check: bool,
    },
    /// Print semantic and provenance hashes for a contract set (CI cache keys)
    Hash {
        /// Directory or file containing contract markdown (recursive)
        path: PathBuf,
        /// Output format
        #[arg(long, default_value = "text", value_enum)]
        format: OutputFormat,
    },
    /// List parsed contracts without generating output
    List {
        /// Directory or file containing contract markdown (recursive)
        path: PathBuf,
    },
    /// Structural diff between two contract sets, plus heuristic conflict candidates
    Diff {
        /// Baseline contract directory or file
        base: PathBuf,
        /// New contract directory or file
        new: PathBuf,
        /// Exit with code 1 if diff is not expansion-only (contracts/conditions removed)
        #[arg(long)]
        strict: bool,
    },
    /// Store contract documents under a project slug for incremental re-verification
    Store {
        /// Directory or file containing contract markdown (recursive)
        path: PathBuf,
        /// Project slug to store contracts under (normalized to lowercase-hyphen)
        #[arg(long)]
        project: String,
        /// Root directory of the contract store
        #[arg(long, default_value = "tauto-store")]
        store_root: PathBuf,
        /// Output format
        #[arg(long, default_value = "text", value_enum)]
        format: OutputFormat,
    },
}

fn main() {
    let cli = Cli::parse();

    if let Err(e) = check_lean_available() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }

    let result = match cli.command {
        Commands::Verify { path, output, strict, format, model, lean_check } => {
            run_verify(&path, &output, strict, &format, &model, lean_check)
        }
        Commands::Hash { path, format } => run_hash(&path, &format),
        Commands::List { path } => run_list(&path),
        Commands::Diff { base, new, strict } => run_diff(&base, &new, strict),
        Commands::Store { path, project, store_root, format } => {
            run_store(&path, &project, &store_root, &format)
        }
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run_verify(
    path: &PathBuf,
    output: &PathBuf,
    strict: bool,
    format: &OutputFormat,
    model: &Option<String>,
    lean_check: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let (contract_set, parse_errors, file_count) = parse_contracts(path)?;

    if contract_set.contracts.is_empty() {
        eprintln!("No contracts parsed ({parse_errors} errors).");
        return Ok(());
    }

    let conflicts = find_conflict_candidates(&contract_set);
    let workspace = generate_lean_workspace(&contract_set);
    let diagnostics = scan_lean_workspace(&workspace);
    write_lean_workspace(&workspace, output)?;

    let slm_result = if let Some(model_name) = model {
        Some(attempt_slm_proofs(&workspace, output, model_name)?)
    } else {
        None
    };

    let lake_result = if lean_check {
        let r = run_lake_build(output).map_err(|e| match e {
            LakeError::NotFound => Box::<dyn std::error::Error>::from(e.to_string()),
            LakeError::Io(io) => Box::<dyn std::error::Error>::from(io),
        })?;
        Some(r)
    } else {
        None
    };

    if *format == OutputFormat::Json {
        let mut json = serde_json::json!({
            "contracts": contract_set.contracts.len(),
            "files": file_count,
            "conflicts": conflicts.iter().map(|c| serde_json::json!({
                "key_a": c.key_a,
                "key_b": c.key_b,
                "reason": c.reason,
            })).collect::<Vec<_>>(),
            "sorry_count": diagnostics.len(),
            "workspace": output.display().to_string(),
        });
        if let Some(ref slm) = slm_result {
            json["slm_model"] = serde_json::Value::String(model.as_deref().unwrap_or("").to_owned());
            json["slm_sorry_count"] = serde_json::json!(slm.remaining_sorrys);
        }
        if let Some(ref lake) = lake_result {
            json["lean_build_success"] = serde_json::json!(lake.success);
        }
        println!("{}", serde_json::to_string_pretty(&json)?);
        if strict && !diagnostics.is_empty() {
            std::process::exit(1);
        }
        return Ok(());
    }

    println!("Parsed {} contract(s) from {} file(s).", contract_set.contracts.len(), file_count);

    if !conflicts.is_empty() {
        println!();
        println!("Conflict candidates ({}):", conflicts.len());
        for c in &conflicts {
            println!("  [conflict] {} ↔ {}", c.key_a, c.key_b);
            println!("    {}", c.reason);
        }
        println!("  Note: heuristic — Lean proof required for confirmation.");
    }

    println!();
    println!("Lean workspace written to {}.", output.display());

    if diagnostics.is_empty() {
        println!("Safety scan: clean.");
    } else {
        for diag in &diagnostics {
            println!(
                "[{}] {}:{} — {}",
                diag.category,
                diag.document_path.as_deref().unwrap_or("?"),
                diag.line.map(|l| l.to_string()).unwrap_or_else(|| "?".to_owned()),
                diag.message
            );
        }
        println!("{} safety finding(s) — sorry stubs require proof.", diagnostics.len());
        if strict {
            std::process::exit(1);
        }
    }

    if let Some(ref slm) = slm_result {
        println!();
        let resolved = diagnostics.len().saturating_sub(slm.remaining_sorrys);
        println!(
            "SLM ({}) proof attempt: {}/{} sorry stubs resolved.",
            model.as_deref().unwrap_or("?"),
            resolved,
            diagnostics.len()
        );
        println!("SLM output written alongside workspace (*.slm.lean).");
    }

    if let Some(ref lake) = lake_result {
        println!();
        if lake.success {
            println!("Lean build: OK — workspace compiles.");
        } else {
            println!("Lean build: FAILED");
            if !lake.stderr.is_empty() {
                for line in lake.stderr.lines().take(20) {
                    println!("  {line}");
                }
            }
        }
    }

    Ok(())
}

fn run_hash(
    path: &PathBuf,
    format: &OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let (contract_set, parse_errors, file_count) = parse_contracts(path)?;

    if contract_set.contracts.is_empty() {
        eprintln!("No contracts parsed ({parse_errors} errors).");
        return Ok(());
    }

    if *format == OutputFormat::Json {
        let json = serde_json::json!({
            "contracts": contract_set.contracts.len(),
            "files": file_count,
            "semantic": semantic_contract_set_hash(&contract_set),
            "provenance": contract_set_hash(&contract_set),
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
        return Ok(());
    }

    println!("contracts : {}", contract_set.contracts.len());
    println!("files     : {file_count}");
    println!("semantic  : {}", semantic_contract_set_hash(&contract_set));
    println!("provenance: {}", contract_set_hash(&contract_set));

    Ok(())
}

fn run_list(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let (contract_set, parse_errors, file_count) = parse_contracts(path)?;

    println!(
        "{} contract(s) from {} file(s) ({parse_errors} parse error(s)):",
        contract_set.contracts.len(),
        file_count
    );
    for c in &contract_set.contracts {
        let src = c
            .source
            .as_ref()
            .map(|s| format!("  [{}:{}]", s.document_path, s.start_line))
            .unwrap_or_default();
        println!("  {}/{}/{}{src}", c.entity, c.operation, c.case);
    }

    Ok(())
}

fn run_diff(
    base_path: &PathBuf,
    new_path: &PathBuf,
    strict: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let (base_set, _, base_files) = parse_contracts(base_path)?;
    let (new_set, _, new_files) = parse_contracts(new_path)?;

    println!(
        "Baseline: {} contract(s) from {} file(s)",
        base_set.contracts.len(),
        base_files
    );
    println!("New:      {} contract(s) from {} file(s)", new_set.contracts.len(), new_files);
    println!();

    let diff = compare(&base_set, &new_set);

    if diff.is_empty() {
        println!("No structural changes.");
    } else {
        for c in &diff.added {
            println!("+ {}/{}/{}", c.entity, c.operation, c.case);
        }
        for c in &diff.removed {
            println!("- {}/{}/{}", c.entity, c.operation, c.case);
        }
        for m in &diff.modified {
            println!("~ {}", m.key.to_display());
            for cond in &m.requires_added {
                println!("    requires +  {}", render_cond(cond));
            }
            for cond in &m.requires_removed {
                println!("    requires -  {}", render_cond(cond));
            }
            for cond in &m.ensures_added {
                println!("    ensures  +  {}", render_cond(cond));
            }
            for cond in &m.ensures_removed {
                println!("    ensures  -  {}", render_cond(cond));
            }
            for s in &m.preserves_added {
                println!("    preserves +  {s}");
            }
            for s in &m.preserves_removed {
                println!("    preserves -  {s}");
            }
            for s in &m.assumes_added {
                println!("    assumes +  {s}");
            }
            for s in &m.assumes_removed {
                println!("    assumes -  {s}");
            }
        }
        println!();
    }

    println!("Expansion only: {}", if diff.is_expansion_only { "yes" } else { "no" });

    // Conflict candidates: only those involving changed/new contracts
    let changed_keys: std::collections::HashSet<String> = diff
        .added
        .iter()
        .map(|c| format!("{}/{}/{}", c.entity, c.operation, c.case))
        .chain(diff.modified.iter().map(|m| m.key.to_display()))
        .collect();

    if !changed_keys.is_empty() {
        let all_candidates = find_conflict_candidates(&new_set);
        let relevant: Vec<_> = all_candidates
            .iter()
            .filter(|c| changed_keys.contains(&c.key_a) || changed_keys.contains(&c.key_b))
            .collect();
        if !relevant.is_empty() {
            println!();
            println!("Conflict candidates involving changed contracts ({}):", relevant.len());
            for c in &relevant {
                println!("  [conflict] {} ↔ {}", c.key_a, c.key_b);
                println!("    {}", c.reason);
            }
            println!("  Note: heuristic — Lean proof required for confirmation.");
        }
    }

    if strict && !diff.is_expansion_only {
        std::process::exit(1);
    }

    Ok(())
}

struct SlmAttemptResult {
    remaining_sorrys: usize,
}

fn attempt_slm_proofs(
    workspace: &LeanWorkspace,
    workspace_path: &PathBuf,
    model_name: &str,
) -> Result<SlmAttemptResult, Box<dyn std::error::Error>> {
    let provider: Box<dyn SlmCodeGenerator> = match model_name {
        "deepseek" => Box::new(DeepSeekProvider::from_env()?),
        name => return Err(format!("unknown model '{name}'; supported: deepseek").into()),
    };

    let mut remaining_sorrys = 0usize;
    for file in workspace.files.iter().filter(|f| f.content.contains("sorry")) {
        let mut context = std::collections::BTreeMap::new();
        context.insert("lean_stub".to_owned(), file.content.clone());

        let request = CodeGenerationRequest {
            contract_set: tauto::contract_ir::ContractSet::new(vec![]),
            target_language: "lean4".to_owned(),
            artifact_kind: ArtifactKind::ProofAttempt,
            context,
        };

        match provider.generate_code_from_ast(&request) {
            Ok(artifact) => {
                let slm_path = workspace_path.join(format!("{}.slm.lean", file.path));
                if let Some(parent) = slm_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&slm_path, &artifact.content)?;
                remaining_sorrys += artifact.content.matches("sorry").count();
            }
            Err(e) => {
                eprintln!("SLM error for {}: {e}", file.path);
            }
        }
    }

    Ok(SlmAttemptResult { remaining_sorrys })
}

fn run_store(
    path: &PathBuf,
    project: &str,
    store_root: &PathBuf,
    format: &OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let files = collect_markdown_files(path)?;
    if files.is_empty() {
        eprintln!("No markdown files found in {}", path.display());
        return Ok(());
    }

    let mut stored_paths = Vec::new();
    for file_path in &files {
        let content = std::fs::read_to_string(file_path)?;
        // Preserve relative path under the scan root so files with identical basenames
        // in different subdirectories are stored at distinct paths (prevents silent overwrites).
        let doc_path = if path.is_file() {
            file_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown.md")
                .to_owned()
        } else {
            file_path
                .strip_prefix(path)
                .ok()
                .and_then(|p| p.to_str())
                .unwrap_or_else(|| {
                    file_path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown.md")
                })
                .to_owned()
        };
        let stem = std::path::Path::new(&doc_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        let title = {
            let mut chars = stem.chars();
            match chars.next() {
                None => stem.to_owned(),
                Some(c) => c.to_uppercase().to_string() + chars.as_str(),
            }
        };
        let doc = ContractDocument::new(project, &doc_path, title, content);
        let stored = save_document(store_root, &doc)?;
        stored_paths.push(stored.display().to_string());
    }

    if *format == OutputFormat::Json {
        let json = serde_json::json!({
            "project": project,
            "stored": stored_paths,
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
        return Ok(());
    }

    println!("Stored {} file(s) for project '{project}':", stored_paths.len());
    for p in &stored_paths {
        println!("  {p}");
    }

    Ok(())
}

fn render_cond(c: &Condition) -> String {
    format!("{} {} {}", c.left.value, c.operator, c.right.value)
}

fn parse_contracts(
    path: &PathBuf,
) -> Result<(ContractSet, usize, usize), Box<dyn std::error::Error>> {
    let files = collect_markdown_files(path)?;
    if files.is_empty() {
        eprintln!("No markdown files found in {}", path.display());
        return Ok((ContractSet::new(vec![]), 0, 0));
    }

    let mut contracts = Vec::new();
    let mut parse_errors = 0usize;

    for file_path in &files {
        let content = std::fs::read_to_string(file_path)?;
        let doc_path = file_path.display().to_string();
        for block in &extract_contract_blocks(&content, &doc_path) {
            let result = parse_contract_block(block);
            for diag in &result.diagnostics {
                eprintln!(
                    "[{}] {}:{} — {}",
                    diag.category,
                    diag.document_path.as_deref().unwrap_or("?"),
                    diag.line.map(|l| l.to_string()).unwrap_or_else(|| "?".to_owned()),
                    diag.message
                );
                parse_errors += 1;
            }
            if let Some(contract) = result.contract {
                contracts.push(contract);
            }
        }
    }

    Ok((ContractSet::new(contracts), parse_errors, files.len()))
}

fn collect_markdown_files(path: &PathBuf) -> std::io::Result<Vec<PathBuf>> {
    if path.is_file() {
        return Ok(vec![path.clone()]);
    }
    let mut files = Vec::new();
    collect_recursive(path, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_recursive(dir: &PathBuf, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let p = entry.path();
        if p.is_dir() {
            collect_recursive(&p, files)?;
        } else if p.extension().map(|e| e == "md").unwrap_or(false) {
            files.push(p);
        }
    }
    Ok(())
}
