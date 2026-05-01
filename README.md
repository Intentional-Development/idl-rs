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
