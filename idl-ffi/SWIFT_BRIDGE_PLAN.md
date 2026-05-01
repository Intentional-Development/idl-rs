# Swift Bridge Plan — IDL FFI Surface

**For:** Banner (Swift/SwiftUI macOS Workbench developer)  
**Status:** Design sketch — no implementation yet  
**Wave:** 7 (planning), 8 (implementation)

---

## Overview

The `idl-ffi` crate will expose `idl-core` functionality to Swift through a C-compatible FFI layer. This allows the native macOS Workbench app to parse, analyze, and manipulate IDL documents without embedding Node.js or calling out to CLI processes.

---

## Architecture

```
┌─────────────────────────────┐
│ SwiftUI macOS Workbench     │
│ (Banner's app)              │
└─────────────┬───────────────┘
              │ Swift calls
              ▼
┌─────────────────────────────┐
│ Swift wrapper (auto-gen)    │
│ (swift-bridge output)       │
└─────────────┬───────────────┘
              │ C ABI
              ▼
┌─────────────────────────────┐
│ idl-ffi (Rust)              │
│ - Parse IDL                 │
│ - Semantic graph queries    │
│ - Drift analysis            │
│ - Error bridging            │
└─────────────┬───────────────┘
              │
              ▼
┌─────────────────────────────┐
│ idl-core (Rust)             │
│ Pure Rust, no I/O deps      │
└─────────────────────────────┘
```

---

## FFI Approach

**Recommended: `swift-bridge`**

- Auto-generates Swift and Rust glue code
- Type-safe bridging (Rust `String` ↔ Swift `String`, etc.)
- Handles memory management (no manual retain/release)
- Supports async/await (if needed later)

**Alternative: `cbindgen`**

- C header generation only
- Requires manual Swift wrapper
- More control, more boilerplate

**Decision:** Use `swift-bridge` unless you need bare-metal control. The ergonomics win is significant.

---

## Core FFI Surface (v1)

### Parse IDL

```rust
// Rust (idl-ffi)
#[swift_bridge::bridge]
mod ffi {
    extern "Rust" {
        fn parse_idl_string(input: String) -> Result<IdlDocument, String>;
        fn parse_idl_file(path: String) -> Result<IdlDocument, String>;
    }
}
```

```swift
// Swift usage
do {
    let doc = try parse_idl_file("/path/to/api.idl")
    print("Parsed \(doc.blocks.count) blocks")
} catch {
    print("Parse error: \(error)")
}
```

---

### Query Semantic Graph

```rust
// Rust (idl-ffi)
#[swift_bridge::bridge]
mod ffi {
    extern "Rust" {
        type SemanticGraph;
        
        fn build_semantic_graph(doc: &IdlDocument) -> SemanticGraph;
        fn find_node_by_id(graph: &SemanticGraph, id: String) -> Option<GraphNode>;
        fn find_edges_from(graph: &SemanticGraph, node_id: String) -> Vec<GraphEdge>;
    }
}
```

```swift
// Swift usage
let graph = build_semantic_graph(doc)
if let node = find_node_by_id(graph, "User") {
    let edges = find_edges_from(graph, "User")
    print("User has \(edges.count) relationships")
}
```

---

### Drift Detection

```rust
// Rust (idl-ffi)
#[swift_bridge::bridge]
mod ffi {
    extern "Rust" {
        fn detect_drift(
            idl_path: String,
            code_path: String,
            language: String
        ) -> Result<DriftReport, String>;
    }
}
```

```swift
// Swift usage
let report = try detect_drift(
    idlPath: "/path/to/idl",
    codePath: "/path/to/generated",
    language: "node"
)
print("Found \(report.drifts.count) drifts")
```

---

## Type Mapping

| Rust Type | Swift Type | Notes |
|-----------|------------|-------|
| `String` | `String` | Auto-bridged |
| `Vec<T>` | `[T]` | Auto-bridged if `T` is bridgeable |
| `HashMap<K,V>` | `Dictionary<K,V>` | Auto-bridged |
| `Option<T>` | `T?` | Auto-bridged |
| `Result<T,E>` | `throws T` | Error bridging via `swift_bridge` |
| `&str` | `RustStr` | Borrowed string (use sparingly) |

---

## Memory Model

**Ownership:**

- **Opaque types** (e.g., `SemanticGraph`): Rust owns, Swift holds pointer. Swift calls `drop_semantic_graph()` when done.
- **Copyable types** (e.g., `String`, `Vec<String>`): Cloned at FFI boundary. Safe but has overhead.
- **Zero-copy** (advanced): Use `&[u8]` slices for large payloads. Requires lifetime management.

**Recommendation for v1:** Clone strings and small collections. Optimize later if profiling shows FFI overhead.

---

## Error Handling

```rust
// Rust (idl-ffi)
#[swift_bridge::bridge]
mod ffi {
    extern "Rust" {
        fn parse_idl_string(input: String) -> Result<IdlDocument, IdlError>;
    }
    
    #[swift_bridge(swift_repr = "struct")]
    struct IdlError {
        message: String,
        line: Option<usize>,
        column: Option<usize>,
    }
}
```

```swift
// Swift usage
do {
    let doc = try parse_idl_string(input)
} catch let error as IdlError {
    print("Parse error at \(error.line):\(error.column): \(error.message)")
}
```

---

## Build Integration

### Xcode Setup

1. **Add Rust library to Xcode target:**
   - Build `idl-ffi` as `staticlib` (for App Store) or `cdylib` (for dev)
   - Link `libidl_ffi.a` in Xcode project
   - Add auto-generated Swift bridge file to project

2. **Build script phase:**
   ```bash
   #!/bin/bash
   cd "${PROJECT_DIR}/../idl-rs/idl-ffi"
   cargo build --release --target aarch64-apple-darwin
   cp target/aarch64-apple-darwin/release/libidl_ffi.a "${BUILT_PRODUCTS_DIR}/"
   ```

3. **Universal binary (Intel + Apple Silicon):**
   ```bash
   cargo build --release --target aarch64-apple-darwin
   cargo build --release --target x86_64-apple-darwin
   lipo -create \
     target/aarch64-apple-darwin/release/libidl_ffi.a \
     target/x86_64-apple-darwin/release/libidl_ffi.a \
     -output libidl_ffi_universal.a
   ```

---

## Open Questions for Banner

1. **Async/sync preference:** Should FFI calls block the main thread, or do you need async Swift wrappers?
2. **Incremental parsing:** Do you need to re-parse the full IDL document on every keystroke, or can we provide incremental updates?
3. **Syntax highlighting:** Should the Rust parser expose token positions for IDE features?
4. **Diagnostics:** Do you want real-time validation errors (as-you-type), or batch validation on save?

---

## Timeline

- **Wave 7 (now):** This design sketch
- **Wave 8:** Implement basic FFI (`parse_idl_string`, error bridging)
- **Wave 9:** Semantic graph queries, drift detection
- **Wave 10:** Performance tuning (zero-copy, incremental parsing if needed)

---

## References

- [swift-bridge docs](https://github.com/chinedufn/swift-bridge)
- [cbindgen docs](https://github.com/mozilla/cbindgen)
- [Apple: Using Swift with C and Objective-C](https://developer.apple.com/documentation/swift/using-swift-with-cocoa-and-objective-c)

---

**Next step:** Banner confirms UI needs, Stark implements FFI stubs in Wave 8.
