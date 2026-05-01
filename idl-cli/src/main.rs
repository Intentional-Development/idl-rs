//! `idl` — IDL command-line interface (Rust).
//!
//! Wave 8 P1 surface: `validate`, `validate-schema`, `change`, `init`,
//! plus carried-over legacy commands `extract`, `emit`, `drift`, `parse`.
//!
//! The CLI is built on `clap` with a subcommand-per-verb layout. Subcommand
//! bodies live in `src/commands/*.rs`; this file is the dispatcher.

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;
use tracing_subscriber::{fmt, EnvFilter};

mod commands;
mod graph_build;

use commands::{change, emit, extract, init, parse, validate, validate_schema, drift};

#[derive(Parser, Debug)]
#[command(
    name = "idl",
    version,
    about = "IDL (Intentional Development Language) — Rust CLI",
    long_about = "Parse, validate, and scaffold IDL projects.\n\
                  Wave 8 P1 surface: validate · validate-schema · init · change · extract.\n\
                  Subcommands operate on the standard `intent/` layout (see IDL/docs/intent-folder-spec.md)."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose (debug-level) logging.
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Parse and validate an IDL project (kernel-aware graph + constraints + loss report).
    #[command(long_about = "Parse the IDL file at `path` (default: intent/project.idl, then project.idl), \n\
        lift it into the kernel-aware semantic graph, and run the default 6 constraints.\n\
        Reports semantic-loss coverage (P0.7). Exit code: 0 ok, 1 errors, 2 warnings (non-strict).")]
    Validate {
        /// IDL file (default: intent/project.idl, then project.idl).
        path: Option<PathBuf>,

        /// Treat warnings as errors and reject non-kernel kinds.
        #[arg(long)]
        strict: bool,

        /// Emit the report as JSON instead of human-readable text.
        #[arg(long)]
        json: bool,
    },

    /// Validate a JSON graph file against `semantic-graph.schema.json` (v0.1.0).
    ValidateSchema {
        /// Path to a JSON graph file.
        graph: PathBuf,

        /// Optional override for the schema path.
        #[arg(long)]
        schema: Option<PathBuf>,

        /// Emit the report as JSON.
        #[arg(long)]
        json: bool,
    },

    /// Initialize an IDL project layout (`intent/` scaffold).
    Init {
        /// Create a fresh greenfield project.
        #[arg(long, conflicts_with = "brownfield")]
        greenfield: bool,

        /// Create a brownfield project (paired with --source).
        #[arg(long, requires = "source")]
        brownfield: bool,

        /// Source directory for brownfield extraction.
        #[arg(long)]
        source: Option<PathBuf>,

        /// Target directory (default: current working directory).
        #[arg(long, default_value = ".")]
        dir: PathBuf,
    },

    /// Manage proposed changes (P0.4 changes folder).
    Change {
        #[command(subcommand)]
        action: ChangeAction,
    },

    /// Brownfield extraction (scaffold; full impl in next pass).
    Extract {
        /// Source directory.
        #[arg(short, long)]
        source: Option<PathBuf>,

        /// Output directory.
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Source language hint.
        #[arg(short, long)]
        language: Option<String>,
    },

    /// Emit code from IDL specification (legacy).
    Emit {
        #[arg(short, long)]
        idl_dir: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(short, long)]
        target: String,
    },

    /// Compare IDL spec with code and detect drift (legacy).
    Drift {
        #[arg(short, long)]
        idl_dir: PathBuf,
        #[arg(short, long)]
        generated: PathBuf,
        #[arg(short, long)]
        compare: String,
        #[arg(short, long)]
        language: String,
    },

    /// Parse IDL files and dump AST (legacy).
    Parse {
        #[arg(short, long)]
        path: PathBuf,
        #[arg(short, long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
enum ChangeAction {
    /// Scaffold a new change folder under `intent/changes/NNNN-<slug>/`.
    New {
        /// Kebab-case slug for the change (e.g. `add-bookings`).
        slug: String,
    },
    /// List change folders and their state.
    List,
    /// Move a change from `draft` → `proposed`.
    Propose {
        /// Change id (NNNN-slug or just NNNN).
        id: String,
    },
    /// Accept a proposed change into `project.idl`.
    Accept {
        id: String,
    },
    /// Reject a proposed change with a reason.
    Reject {
        id: String,
        #[arg(long)]
        reason: String,
    },
    /// Show the semantic diff for a change.
    Diff {
        id: String,
    },
    /// Validate a change folder (delta well-formed, references resolve).
    Validate {
        id: String,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    match dispatch(cli.command) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::from(1)
        }
    }
}

fn init_tracing(verbose: bool) {
    let filter = if verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"))
    };
    let _ = fmt().with_env_filter(filter).with_target(false).try_init();
}

fn dispatch(cmd: Commands) -> Result<ExitCode> {
    match cmd {
        Commands::Validate { path, strict, json } => validate::run(path, strict, json),
        Commands::ValidateSchema { graph, schema, json } => {
            validate_schema::run(graph, schema, json)
        }
        Commands::Init { greenfield, brownfield, source, dir } => {
            init::run(dir, greenfield, brownfield, source)
        }
        Commands::Change { action } => match action {
            ChangeAction::New { slug } => change::new(slug),
            ChangeAction::List => change::list(),
            ChangeAction::Propose { id } => change::stub("propose", &id),
            ChangeAction::Accept { id } => change::stub("accept", &id),
            ChangeAction::Reject { id, reason } => {
                change::stub_with(&format!("reject {id} (reason: {reason})"))
            }
            ChangeAction::Diff { id } => change::stub("diff", &id),
            ChangeAction::Validate { id } => change::stub("validate", &id),
        },
        Commands::Extract { source, output, language } => extract::run(source, output, language),
        Commands::Emit { idl_dir, output, target } => emit::run(idl_dir, output, target),
        Commands::Drift { idl_dir, generated, compare, language } => {
            drift::run(idl_dir, generated, compare, language)
        }
        Commands::Parse { path, json } => parse::run(path, json),
    }
}
