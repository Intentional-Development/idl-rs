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
mod diagnostic_formatter;
mod graph_build;
mod proposals;

use commands::{change, emit, extract, init, interview, parse, propose, validate, validate_schema, drift};

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
        Reports semantic-loss coverage (P0.7). Exit code: 0 ok, 1 errors, 2 warnings (non-strict).\n\
        With --anchors, validates source_anchors against the filesystem under --source.")]
    Validate {
        /// IDL file (default: intent/project.idl, then project.idl).
        path: Option<PathBuf>,

        /// Treat warnings as errors and reject non-kernel kinds.
        #[arg(long)]
        strict: bool,

        /// Emit the report as JSON instead of human-readable text.
        #[arg(long)]
        json: bool,

        /// Run anchor validation instead of IDL validation. The path argument
        /// is treated as a graph JSON file. Requires --source.
        #[arg(long, requires = "source")]
        anchors: bool,

        /// Source root (workspace) for anchor resolution. Used with --anchors.
        #[arg(long)]
        source: Option<PathBuf>,
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

        /// Rewrite `source_anchors[].uri` prefixes in a graph file. Pass two
        /// values: <old-prefix> <new-prefix>. The graph path is taken from
        /// --output (or --source if --output absent).
        #[arg(long, num_args = 2, value_names = ["OLD_PREFIX", "NEW_PREFIX"])]
        rewrite_anchors: Option<Vec<String>>,

        /// When rewriting, edit the input file in-place (preserving original
        /// to a sibling `*.legacy.json`). Default writes a sibling
        /// `*.rewritten.json` and leaves the input alone.
        #[arg(long)]
        in_place: bool,
    },

    /// Emit code from an extracted graph (P1.4 codegen).
    ///
    /// `idl emit <target> <graph.json> --out <dir>`
    Emit {
        /// Target language: `rust`, `typescript`, `openapi`.
        target: String,
        /// Path to the schema-shaped graph JSON file.
        graph: PathBuf,
        /// Output directory.
        #[arg(long)]
        out: PathBuf,
    },

    /// Detect drift (P1.7).
    ///
    /// `idl drift graph <baseline> <current>`  — graph-vs-graph
    /// `idl drift code  <graph> --source <p>`  — graph-vs-code
    Drift {
        #[command(subcommand)]
        action: DriftAction,
    },

    /// Parse IDL files and dump AST (legacy).
    Parse {
        #[arg(short, long)]
        path: PathBuf,
        #[arg(short, long)]
        json: bool,
    },

    /// Multi-round greenfield interview producing a proposed graph delta.
    ///
    /// `idl interview new --topic "todo app"`
    /// `idl interview continue <session-id>`
    /// `idl interview accept <session-id>`
    /// `idl interview list`
    /// `idl interview show <session-id>`
    Interview {
        #[command(subcommand)]
        action: InterviewAction,
    },

    /// Create a new proposal from a change specification.
    ///
    /// `idl propose <graph-path> <change-spec.json>`
    Propose {
        /// Path to the target graph file.
        graph: PathBuf,
        /// Path to the change specification JSON file.
        change_spec: PathBuf,
    },

    /// Manage proposals (list, accept, reject).
    ///
    /// `idl proposals list`
    /// `idl proposals accept <id>`
    /// `idl proposals reject <id> --reason "..."`
    Proposals {
        #[command(subcommand)]
        action: ProposalsAction,
    },
}

#[derive(Subcommand, Debug)]
enum InterviewAction {
    /// Start a new session and run round 1.
    New {
        #[arg(long)]
        topic: String,
        #[arg(long, default_value_t = 5)]
        rounds: u32,
    },
    /// Run the next round of an existing session.
    Continue {
        session_id: String,
    },
    /// Promote the accumulated delta into a proposed change folder.
    Accept {
        session_id: String,
    },
    /// List all sessions.
    List,
    /// Print the transcript and accumulated graph for a session.
    Show {
        session_id: String,
    },
}

#[derive(Subcommand, Debug)]
enum ProposalsAction {
    /// List all proposals.
    List,
    /// Accept a proposal and apply it to the target graph.
    Accept {
        /// Proposal ID (or prefix).
        id: String,
    },
    /// Reject a proposal with a reason.
    Reject {
        /// Proposal ID (or prefix).
        id: String,
        /// Rejection reason.
        #[arg(long)]
        reason: String,
    },
}

#[derive(Subcommand, Debug)]
enum DriftAction {
    /// Compare two graph JSON files.
    Graph {
        baseline: PathBuf,
        current: PathBuf,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        markdown: bool,
    },
    /// Re-anchor a graph against a source code root.
    Code {
        graph: PathBuf,
        /// Source root for anchor resolution. May be specified multiple times.
        ///
        /// Forms:
        /// * `--source <path>` — fallback root for `repo://*` URIs and bare
        ///   relative paths.
        /// * `--source <corpus>=<path>` — named corpus mapping. Any
        ///   `repo://<corpus>/...` URI is routed under `<path>`.
        #[arg(long)]
        source: Vec<PathBuf>,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        markdown: bool,
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
        Commands::Validate { path, strict, json, anchors, source } => {
            if anchors {
                let graph = path.ok_or_else(|| {
                    anyhow::anyhow!("--anchors requires a graph JSON path argument")
                })?;
                let src = source.ok_or_else(|| {
                    anyhow::anyhow!("--anchors requires --source <root>")
                })?;
                commands::validate_anchors::run(graph, src, json)
            } else {
                validate::run(path, strict, json)
            }
        }
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
        Commands::Extract { source, output, language, rewrite_anchors, in_place } => {
            extract::run(source, output, language, rewrite_anchors, in_place)
        }
        Commands::Emit { target, graph, out } => emit::run(target, graph, out),
        Commands::Drift { action } => match action {
            DriftAction::Graph { baseline, current, json, markdown } => {
                let fmt = if json {
                    drift::OutputFormat::Json
                } else if markdown {
                    drift::OutputFormat::Markdown
                } else {
                    drift::OutputFormat::Human
                };
                drift::run_graph(baseline, current, fmt)
            }
            DriftAction::Code { graph, source, json, markdown } => {
                let fmt = if json {
                    drift::OutputFormat::Json
                } else if markdown {
                    drift::OutputFormat::Markdown
                } else {
                    drift::OutputFormat::Human
                };
                drift::run_code(graph, source, fmt)
            }
        },
        Commands::Parse { path, json } => parse::run(path, json),
        Commands::Interview { action } => match action {
            InterviewAction::New { topic, rounds } => interview::run_new(topic, rounds),
            InterviewAction::Continue { session_id } => interview::run_continue(session_id),
            InterviewAction::Accept { session_id } => interview::run_accept(session_id),
            InterviewAction::List => interview::run_list(),
            InterviewAction::Show { session_id } => interview::run_show(session_id),
        },
        Commands::Propose { graph, change_spec } => propose::run(graph, change_spec),
        Commands::Proposals { action } => match action {
            ProposalsAction::List => commands::proposals::list(),
            ProposalsAction::Accept { id } => commands::proposals::accept(id),
            ProposalsAction::Reject { id, reason } => commands::proposals::reject(id, reason),
        },
    }
}
