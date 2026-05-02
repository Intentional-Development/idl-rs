# idl-ffi — C-Compatible FFI for Swift/SwiftUI Workbench

**Status:** W24 scaffold (v0.9.9-rc.3)  
**Target:** macOS Workbench (Banner's Swift app, W25+)

---

## Overview

`idl-ffi` exposes `idl-core` functionality through a C-compatible FFI layer. This allows native Swift applications to parse, analyze, and manipulate IDL documents without embedding Node.js or spawning CLI processes.

---

## FFI Surface

### Core Functions

```c
// Parse IDL graph from directory
char* idl_parse_graph(const char* path);

// Free returned strings (mandatory)
void idl_free_string(char* s);

// Validate graph JSON
int32_t idl_validate_graph(const char* json);

// Classify node behaviors
char* idl_classify_behavior(const char* json);
```

See `include/idl_ffi.h` for full API documentation.

---

## Memory Ownership

**Critical rules:**

1. **Returned strings are owned by the caller**  
   Every `char*` returned by `idl_parse_graph` or `idl_classify_behavior` MUST be freed with `idl_free_string`.

2. **Input strings are borrowed**  
   `const char*` parameters are read-only and not freed by the FFI layer.

3. **Thread safety**  
   All functions are thread-safe. You can call them from any thread.

**Example (C):**
```c
const char* path = "/path/to/idl";
char* result = idl_parse_graph(path);

// Use result...
printf("%s\n", result);

// MUST free!
idl_free_string(result);
```

**Example (Swift):**
```swift
let path = "/path/to/idl"
if let resultPtr = idl_parse_graph(path) {
    let result = String(cString: resultPtr)
    print(result)
    idl_free_string(resultPtr)
}
```

---

## Error Handling

### String-Returning Functions

On error, return JSON with `"error"` key:
```json
{"error": "path does not exist: /bad/path"}
```

The caller MUST still call `idl_free_string` on error results.

### Integer-Returning Functions

`idl_validate_graph` returns error codes:

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Invalid JSON |
| 2 | Invalid schema (e.g., empty graph) |
| 3 | Null pointer |
| 4 | Invalid UTF-8 |

---

## Behavior Classifications

`idl_classify_behavior` returns a JSON map of node_id → behavior:

| Behavior | Description |
|----------|-------------|
| `entity` | Object with `id` property |
| `dto` | Object with properties, no `id` |
| `abstract` | Object with no properties |
| `enum` | Enumeration |
| `function` | Function definition |
| `module` | Module container |

**Example output:**
```json
{
  "User": "entity",
  "Status": "enum",
  "UpdateRequest": "dto"
}
```

---

## Threading Model

All functions are thread-safe and reentrant. However:

- **Do not share pointers across threads** without synchronization.
- **Free strings on the same thread** that received them (recommended, not required).
- **Graph parsing is CPU-intensive** — consider offloading to background threads.

---

## Build Integration

### Static Library

```bash
cargo build --release -p idl-ffi
# Output: target/release/libidl_ffi.a
```

### Dynamic Library

```bash
cargo build --release -p idl-ffi
# Output: target/release/libidl_ffi.dylib (macOS)
```

### Xcode Integration

1. Add `libidl_ffi.a` to Link Binary With Libraries
2. Add `idl-ffi/include` to Header Search Paths
3. Import: `#include "idl_ffi.h"`

For swift-bridge integration (W25), this header will be used to generate Swift wrappers automatically.

---

## Testing

```bash
cargo test -p idl-ffi
```

Tests cover:
- Null pointer handling
- Invalid inputs (bad JSON, nonexistent paths)
- Valid round-trips (parse → classify → free)
- Memory lifecycle (no leaks, no double-frees)

---

## Roadmap

- **W24 (current):** C-compatible FFI scaffold, unit tests
- **W25:** swift-bridge integration, Xcode build scripts
- **W26+:** Incremental parsing, async API, performance tuning

---

## Notes for Banner (Swift Developer)

- This is a **low-level C API**. In W25, we'll add a Swift wrapper layer for ergonomics.
- For now, use `String(cString:)` to bridge C strings to Swift.
- Always pair `idl_parse_graph` with `idl_free_string` — Swift ARC won't free these automatically.

---

**Questions?** Ping Stark in `.squad/decisions/inbox/`.
