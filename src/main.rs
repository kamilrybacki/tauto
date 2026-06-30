use std::path::PathBuf;

use clap::Parser;

use tauto::contract_ir::{ContractSet, semantic_contract_set_hash, contract_set_hash};
use tauto::contract_parser::{extract_contract_blocks, parse_contract_block};
use tauto::lean_gen::{generate_lean_workspace, scan_lean_workspace, write_lean_workspace};

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
    },
    /// Print semantic and provenance hashes for a contract set (useful for cache keys)
    Hash {
        /// Directory or file containing contract markdown (recursive)
        path: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Commands::Verify { path, output } => run_verify(&path, &output),
        Commands::Hash { path } => run_hash(&path),
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run_verify(path: &PathBuf, output: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let (contract_set, parse_errors, file_count) = parse_contracts(path)?;

    if contract_set.contracts.is_empty() {
        eprintln!("No contracts parsed ({parse_errors} errors).");
        return Ok(());
    }

    println!("Parsed {} contract(s) from {} file(s).", contract_set.contracts.len(), file_count);

    let workspace = generate_lean_workspace(&contract_set);
    let diagnostics = scan_lean_workspace(&workspace);
    write_lean_workspace(&workspace, output)?;
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
    }

    Ok(())
}

fn run_hash(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let (contract_set, parse_errors, file_count) = parse_contracts(path)?;

    if contract_set.contracts.is_empty() {
        eprintln!("No contracts parsed ({parse_errors} errors).");
        return Ok(());
    }

    println!("contracts : {}", contract_set.contracts.len());
    println!("files     : {file_count}");
    println!("semantic  : {}", semantic_contract_set_hash(&contract_set));
    println!("provenance: {}", contract_set_hash(&contract_set));

    Ok(())
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
