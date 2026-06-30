use std::path::PathBuf;

use clap::Parser;

use tauto::contract_ir::ContractSet;
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
    /// Parse contracts from markdown files and generate a Lean 4 workspace
    Verify {
        /// Directory or file containing contract markdown
        path: PathBuf,
        /// Where to write the generated Lean workspace (default: ./lean-workspace)
        #[arg(long, default_value = "lean-workspace")]
        output: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Verify { path, output } => {
            if let Err(e) = run_verify(&path, &output) {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
    }
}

fn run_verify(path: &PathBuf, output: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let files = collect_markdown_files(path)?;
    if files.is_empty() {
        eprintln!("No markdown files found in {}", path.display());
        return Ok(());
    }

    let mut contracts = Vec::new();
    let mut parse_errors = 0usize;

    for file_path in &files {
        let content = std::fs::read_to_string(file_path)?;
        let doc_path = file_path.display().to_string();
        let blocks = extract_contract_blocks(&content, &doc_path);
        for block in &blocks {
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

    if contracts.is_empty() {
        eprintln!("No contracts parsed ({parse_errors} errors).");
        return Ok(());
    }

    println!("Parsed {} contract(s) from {} file(s).", contracts.len(), files.len());

    let contract_set = ContractSet::new(contracts);
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

fn collect_markdown_files(path: &PathBuf) -> std::io::Result<Vec<PathBuf>> {
    if path.is_file() {
        return Ok(vec![path.clone()]);
    }
    let mut files = Vec::new();
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let p = entry.path();
        if p.is_file() && p.extension().map(|e| e == "md").unwrap_or(false) {
            files.push(p);
        }
    }
    files.sort();
    Ok(files)
}
