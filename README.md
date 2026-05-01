# IDL — Rust Implementation

**Status:** Wave 7 bootstrap (0.1.0-rust)  
**Mission:** Rewrite IDL toolchain in Rust for performance, FFI, and native platform support

---

## Architecture

```
idl-rs/
├── idl-core/        # Parser, AST, semantic graph, drift engine (pure Rust, no I/O)
├── idl-cli/         # Command-line interface (clap-based)
├── idl-emitters/    # Code generators (Node, Go, Python, Rust)
├── idl-extractors/  # Brownfield extractors (TS, Dart, PHP, JS)
└── idl-ffi/         # C ABI for Swift/SwiftUI macOS Workbench
```

---

## Build

```bash
cargo build --release
cargo test
```

Binary output: `target/release/idl`

---

## Usage

```bash
# Parse and validate IDL
idl parse --path realworld-idl/idl/

# Extract IDL from brownfield code (stub)
idl extract --source ./app --output ./idl --language typescript

# Emit code from IDL (stub)
idl emit --idl-dir ./idl --output ./generated --target node

# Detect drift (stub)
idl drift --idl-dir ./idl --generated ./generated --compare emit --language node
```

### `idl drift code` — Source/IDL drift sweep (Wave 10)

```bash
idl drift code <graph.json> \
    [--source <root>] \
    [--source <name>=<root>] ... \
    [--markdown | --json]
```

**`--source` forms** (repeatable):

- `--source ./app` — bare path; becomes the default root for `repo://` URIs
  with no corpus prefix.
- `--source name=./path` — named corpus root. URIs of the form
  `repo://name/...` resolve under `<path>/...`. Multiple `--source name=...`
  flags map multiple corpora in one invocation (e.g. an app tree plus the
  `IDL/` spec tree).

**Anchor verdicts** (lowercase in markdown/human output):

| Verdict       | Meaning                                                |
|---------------|--------------------------------------------------------|
| `aligned`     | URI resolves and the line range is in bounds.          |
| `missing`     | Source file does not exist.                            |
| `shifted`     | URI resolves but the anchor's line range is invalid.   |
| `new-in-code` | (Reserved) Code feature with no matching IDL anchor.   |

Directory anchors (URIs that resolve to a directory) report `aligned` when
the directory exists; line ranges are ignored. End-line equal to
`file_line_count + 1` is silently clamped (treats trailing-newline EOFs as
in-bounds).

**Exit codes:**

| Code | Meaning                                              |
|------|------------------------------------------------------|
| `0`  | All anchors `aligned`.                               |
| `1`  | One or more anchors `missing` (file not found).      |
| `2`  | One or more anchors `new-in-code`.                   |
| `3`  | One or more anchors `shifted` (line range invalid).  |

When multiple non-zero categories are present, the highest-priority code is
returned in the order: `missing` (1) > `new-in-code` (2) > `shifted` (3).

---

## Parser Status (Wave 7)

**Implemented:**
- ✅ Module envelope parsing (`idl_version`, `module`, metadata)
- ✅ Core block types: `intent`, `scope`, `entity`, `event`, `rule`, `invariant`, `api`
- ✅ Extension block fallback (future-proofs against unknown block types)
- ✅ Basic property/field parsing
- ✅ String literals, identifiers, arrays
- ✅ Comment stripping

**TODO (Wave 8+):**
- Nested sub-blocks (`properties`, `payload`, `endpoint`, `transitions`)
- Type expression parsing (generics, collections, optionals)
- Complete block type coverage (27 blocks in spec)
- Error recovery (continue parsing after syntax error)
- Source location tracking (for IDE features)

**Acceptance:** Parses all `.idl` files under `realworld-idl/idl/` without hard errors (warnings acceptable).

---

## Emitters/Extractors (Wave 8+)

All emit/extract logic is stubbed. The CLI surface matches the TS workbench-cli for compatibility:

- `extract` → brownfield extraction (LLM-assisted)
- `emit` → code generation (AST → target language)
- `drift` → spec/code divergence detection

---

## FFI for Swift (Wave 8+)

See `idl-ffi/SWIFT_BRIDGE_PLAN.md` for the intended FFI surface. Banner's macOS Workbench will link against `libidl_ffi.a` for native parsing/analysis.

---

## Versioning

- **Rust crate:** `0.1.0` → `1.0.0` (independent from TS workbench-cli)
- **TS workbench-cli:** Frozen at `1.0.0-rc1` (bugfixes only)
- **Feature parity milestone:** When Rust CLI matches TS CLI surface, deprecate TS

---

## Design Principles

1. **Pure Rust core:** `idl-core` has zero I/O dependencies. All file ops live in `idl-cli` or above. This makes FFI bindings clean.
2. **Separation of concerns:** Parser → AST → Semantic graph → Emitters/Drift. Each phase is testable in isolation.
3. **Error as data:** Parse errors are structured (line, column, message), not panics. IDEs need recoverable errors.
4. **Extension blocks:** Unknown block types parse as `Extension` nodes. Forward-compatible with spec evolution.

---

## Development

**Run tests:**
```bash
cargo test --workspace
```

**Lint:**
```bash
cargo clippy --workspace -- -D warnings
```

**Format:**
```bash
cargo fmt --all
```

**Parse real IDL:**
```bash
cargo run --release --bin idl -- parse --path ../realworld-idl/idl/
```

---

## Contributing

See `.squad/decisions/inbox/stark-w7-rust-pivot.md` for the migration plan and architecture decisions.

---

## License

MIT
