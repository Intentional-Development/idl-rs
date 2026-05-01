use clap::{Parser, Subcommand};
use anyhow::{Context, Result};
use std::path::PathBuf;
use tracing::{info, warn};
use tracing_subscriber::{EnvFilter, fmt};

#[derive(Parser)]
#[command(name = "idl")]
#[command(about = "IDL (Intentional Development Language) CLI - Rust implementation", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Extract IDL from brownfield codebase
    Extract {
        /// Source code directory
        #[arg(short, long)]
        source: PathBuf,

        /// Output IDL directory
        #[arg(short, long)]
        output: PathBuf,

        /// Source language (ts, dart, php, js)
        #[arg(short, long)]
        language: String,
    },

    /// Emit code from IDL specification
    Emit {
        /// IDL directory
        #[arg(short, long)]
        idl_dir: PathBuf,

        /// Output directory for generated code
        #[arg(short, long)]
        output: PathBuf,

        /// Target language (node, go, python, rust)
        #[arg(short, long)]
        target: String,
    },

    /// Compare IDL spec with generated/extracted code and detect drift
    Drift {
        /// IDL directory
        #[arg(short, long)]
        idl_dir: PathBuf,

        /// Generated code directory (or source for extraction)
        #[arg(short, long)]
        generated: PathBuf,

        /// Comparison mode (emit or extract)
        #[arg(short, long)]
        compare: String,

        /// Target/source language
        #[arg(short, long)]
        language: String,
    },

    /// Parse and validate IDL files
    Parse {
        /// IDL file or directory
        #[arg(short, long)]
        path: PathBuf,

        /// Output parsed AST as JSON
        #[arg(short, long)]
        json: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    let filter = if cli.verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };

    fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    match cli.command {
        Commands::Extract { source, output, language } => {
            extract_command(source, output, language).await?;
        }
        Commands::Emit { idl_dir, output, target } => {
            emit_command(idl_dir, output, target).await?;
        }
        Commands::Drift { idl_dir, generated, compare, language } => {
            drift_command(idl_dir, generated, compare, language).await?;
        }
        Commands::Parse { path, json } => {
            parse_command(path, json).await?;
        }
    }

    Ok(())
}

async fn extract_command(source: PathBuf, output: PathBuf, language: String) -> Result<()> {
    info!("Extracting IDL from {} ({}) to {}", 
          source.display(), language, output.display());
    
    warn!("⚠️  Extract command not yet implemented");
    warn!("    Brownfield extraction will be ported in Wave 8");
    warn!("    Source language: {}", language);
    warn!("    Source path: {}", source.display());
    warn!("    Output path: {}", output.display());
    
    Ok(())
}

async fn emit_command(idl_dir: PathBuf, output: PathBuf, target: String) -> Result<()> {
    info!("Emitting {} code from {} to {}", 
          target, idl_dir.display(), output.display());
    
    // Read and parse IDL files
    let idl_files = find_idl_files(&idl_dir)?;
    info!("Found {} IDL files", idl_files.len());
    
    for file in &idl_files {
        let content = std::fs::read_to_string(file)
            .with_context(|| format!("Failed to read {}", file.display()))?;
        
        match idl_core::parse_idl(&content) {
            Ok(doc) => {
                info!("✓ Parsed {}: {} blocks", file.display(), doc.blocks.len());
            }
            Err(e) => {
                warn!("✗ Failed to parse {}: {}", file.display(), e);
            }
        }
    }
    
    warn!("⚠️  Emit command not yet implemented");
    warn!("    Code generation will be ported in Wave 8");
    warn!("    Target language: {}", target);
    warn!("    Output path: {}", output.display());
    
    Ok(())
}

async fn drift_command(idl_dir: PathBuf, generated: PathBuf, compare: String, language: String) -> Result<()> {
    info!("Detecting drift between {} and {} (mode: {}, language: {})", 
          idl_dir.display(), generated.display(), compare, language);
    
    warn!("⚠️  Drift command not yet implemented");
    warn!("    Drift detection will be ported in Wave 8");
    warn!("    Comparison mode: {}", compare);
    warn!("    Language: {}", language);
    warn!("    IDL path: {}", idl_dir.display());
    warn!("    Generated/source path: {}", generated.display());
    
    Ok(())
}

async fn parse_command(path: PathBuf, json: bool) -> Result<()> {
    info!("Parsing IDL at {}", path.display());
    
    if path.is_file() {
        parse_file(&path, json)?;
    } else if path.is_dir() {
        let idl_files = find_idl_files(&path)?;
        info!("Found {} IDL files", idl_files.len());
        
        for file in idl_files {
            parse_file(&file, json)?;
        }
    } else {
        anyhow::bail!("Path does not exist: {}", path.display());
    }
    
    Ok(())
}

fn parse_file(file: &PathBuf, json: bool) -> Result<()> {
    let content = std::fs::read_to_string(file)
        .with_context(|| format!("Failed to read {}", file.display()))?;
    
    match idl_core::parse_idl(&content) {
        Ok(doc) => {
            if json {
                let json = serde_json::to_string_pretty(&doc)?;
                println!("{}", json);
            } else {
                info!("✓ {} - version {}, {} blocks", 
                      file.display(), doc.version, doc.blocks.len());
                
                if let Some(module) = &doc.module {
                    info!("  Module: {}", module.name);
                }
                
                for block in &doc.blocks {
                    let block_name = match block {
                        idl_core::Block::Intent(b) => format!("intent {}", b.name),
                        idl_core::Block::Scope(b) => format!("scope {}", b.name),
                        idl_core::Block::Entity(b) => format!("entity {}", b.name),
                        idl_core::Block::Event(b) => format!("event {}", b.name),
                        idl_core::Block::Rule(b) => format!("rule {}", b.name),
                        idl_core::Block::Invariant(b) => format!("invariant \"{}\"", b.name),
                        idl_core::Block::Api(b) => format!("api {}", b.name),
                        idl_core::Block::Extension(b) => format!("{} {}", b.block_type, 
                            b.name.as_ref().unwrap_or(&"(anonymous)".to_string())),
                        _ => "other".to_string(),
                    };
                    info!("  - {}", block_name);
                }
            }
        }
        Err(e) => {
            warn!("✗ Failed to parse {}: {}", file.display(), e);
            return Err(e.into());
        }
    }
    
    Ok(())
}

fn find_idl_files(dir: &PathBuf) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    
    for entry in walkdir::WalkDir::new(dir) {
        let entry = entry?;
        if entry.file_type().is_file() {
            if let Some(ext) = entry.path().extension() {
                if ext == "idl" {
                    files.push(entry.path().to_path_buf());
                }
            }
        }
    }
    
    Ok(files)
}
