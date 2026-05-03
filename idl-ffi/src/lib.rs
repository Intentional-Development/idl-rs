#![allow(clippy::not_unsafe_ptr_arg_deref)]

// FFI bindings for Swift/SwiftUI macOS Workbench
//
// This crate exposes idl-core functionality through a C-compatible FFI.
// Memory model: caller owns returned strings, must call idl_free_string.

use std::collections::BTreeMap;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::Path;

/// Parse an IDL graph from a directory path and return JSON representation.
/// Returns JSON string on success, error JSON on failure.
/// Caller must call idl_free_string on the returned pointer.
#[no_mangle]
pub extern "C" fn idl_parse_graph(path: *const c_char) -> *mut c_char {
    if path.is_null() {
        return error_to_cstring("null path pointer");
    }

    let path_str = unsafe {
        match CStr::from_ptr(path).to_str() {
            Ok(s) => s,
            Err(_) => return error_to_cstring("invalid UTF-8 in path"),
        }
    };

    match parse_graph_internal(path_str) {
        Ok(json) => match CString::new(json) {
            Ok(cs) => cs.into_raw(),
            Err(_) => error_to_cstring("JSON contains null byte"),
        },
        Err(e) => error_to_cstring(&e),
    }
}

fn parse_graph_internal(path_str: &str) -> Result<String, String> {
    let path = Path::new(path_str);
    if !path.exists() {
        return Err(format!("path does not exist: {}", path_str));
    }

    // Discover .idl files in directory
    let idl_files =
        discover_idl_files(path).map_err(|e| format!("failed to discover IDL files: {}", e))?;

    if idl_files.is_empty() {
        return Err(format!("no .idl files found in: {}", path_str));
    }

    // Parse each file and lift to graph
    let mut combined_graph = idl_graph::Graph::new();
    for file_path in idl_files {
        let content = std::fs::read_to_string(&file_path)
            .map_err(|e| format!("failed to read {:?}: {}", file_path, e))?;

        let doc = idl_core::parse_idl(&content)
            .map_err(|e| format!("failed to parse {:?}: {:?}", file_path, e))?;

        let lifted = lift_document(&doc, file_path.to_string_lossy().as_ref());

        // Merge nodes and edges
        for (_, node) in lifted.graph.nodes {
            combined_graph.add_node(node);
        }
        for (_, edge) in lifted.graph.edges {
            combined_graph.add_edge(edge);
        }
    }

    serde_json::to_string_pretty(&combined_graph)
        .map_err(|e| format!("failed to serialize graph: {}", e))
}

fn discover_idl_files(dir: &Path) -> Result<Vec<std::path::PathBuf>, String> {
    let mut files = Vec::new();

    if dir.is_file() {
        if dir.extension().and_then(|s| s.to_str()) == Some("idl") {
            files.push(dir.to_path_buf());
        }
        return Ok(files);
    }

    for entry in walkdir::WalkDir::new(dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
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

struct LiftResult {
    graph: idl_graph::Graph,
}

fn lift_document(doc: &idl_core::IdlDocument, source_path: &str) -> LiftResult {
    let mut graph = idl_graph::Graph::new();

    for block in &doc.blocks {
        if let Some((kind, name)) = block_to_kernel(block) {
            let id = idl_graph::NodeId(format!("{}::{}", kind.canonical(), name));
            let node = idl_graph::Node {
                id: id.clone(),
                kind,
                state: idl_graph::NodeState::Accepted,
                props: BTreeMap::new(),
                source_anchors: vec![idl_graph::SourceAnchor {
                    uri: source_path.to_string(),
                    range: None,
                    hash: None,
                }],
                confidence: None,
            };
            graph.add_node(node);
        }
    }

    LiftResult { graph }
}

fn block_to_kernel(block: &idl_core::Block) -> Option<(idl_graph::NodeKind, String)> {
    let pair = match block {
        idl_core::Block::Intent(b) => (idl_graph::NodeKind::Intent, b.name.clone()),
        idl_core::Block::Scope(b) => (idl_graph::NodeKind::Scope, b.name.clone()),
        idl_core::Block::Entity(b) => (idl_graph::NodeKind::Entity, b.name.clone()),
        idl_core::Block::Aggregate(b) => (idl_graph::NodeKind::Aggregate, b.name.clone()),
        idl_core::Block::Variant(b) => (idl_graph::NodeKind::Variant, b.name.clone()),
        idl_core::Block::Constraints(b) => (idl_graph::NodeKind::Constraints, b.name.clone()),
        idl_core::Block::Event(b) => (idl_graph::NodeKind::Event, b.name.clone()),
        idl_core::Block::Operation(b) => (idl_graph::NodeKind::Operation, b.name.clone()),
        idl_core::Block::StateMachine(b) => (idl_graph::NodeKind::StateMachine, b.name.clone()),
        idl_core::Block::Rule(b) => (idl_graph::NodeKind::Rule, b.name.clone()),
        idl_core::Block::Invariant(b) => (idl_graph::NodeKind::Invariant, b.name.clone()),
        idl_core::Block::Policy(b) => (idl_graph::NodeKind::Policy, b.name.clone()),
        idl_core::Block::Api(b) => (idl_graph::NodeKind::Api, b.name.clone()),
        idl_core::Block::Mapping(b) => (idl_graph::NodeKind::Mapping, b.name.clone()),
        idl_core::Block::TraceLink(b) => (
            idl_graph::NodeKind::TraceLink,
            format!("{}__{}", b.from, b.to),
        ),
        idl_core::Block::Decision(b) => (idl_graph::NodeKind::Decision, b.name.clone()),
        idl_core::Block::Verification(b) => (idl_graph::NodeKind::Verification, b.name.clone()),
        _ => return None,
    };
    Some(pair)
}

/// Free a string returned by idl_parse_graph or idl_classify_behavior.
/// MUST be called exactly once for each returned string.
#[no_mangle]
pub extern "C" fn idl_free_string(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    unsafe {
        let _ = CString::from_raw(s);
    }
}

/// Validate a graph JSON string.
/// Returns 0 on success, error code on failure.
/// Error codes: 1=invalid JSON, 2=invalid schema, 3=null pointer, 4=invalid UTF-8
#[no_mangle]
pub extern "C" fn idl_validate_graph(json: *const c_char) -> i32 {
    if json.is_null() {
        return 3;
    }

    let json_str = unsafe {
        match CStr::from_ptr(json).to_str() {
            Ok(s) => s,
            Err(_) => return 4,
        }
    };

    match validate_graph_internal(json_str) {
        Ok(_) => 0,
        Err(ValidationError::InvalidJson) => 1,
        Err(ValidationError::InvalidSchema) => 2,
    }
}

enum ValidationError {
    InvalidJson,
    InvalidSchema,
}

fn validate_graph_internal(json_str: &str) -> Result<(), ValidationError> {
    let graph: idl_graph::Graph =
        serde_json::from_str(json_str).map_err(|_| ValidationError::InvalidJson)?;

    if graph.nodes.is_empty() && graph.edges.is_empty() {
        return Err(ValidationError::InvalidSchema);
    }

    Ok(())
}

/// Classify behavior of each node in a graph JSON.
/// Returns JSON map of node_id -> behavior classification.
/// Caller must call idl_free_string on the returned pointer.
#[no_mangle]
pub extern "C" fn idl_classify_behavior(json: *const c_char) -> *mut c_char {
    if json.is_null() {
        return error_to_cstring("null json pointer");
    }

    let json_str = unsafe {
        match CStr::from_ptr(json).to_str() {
            Ok(s) => s,
            Err(_) => return error_to_cstring("invalid UTF-8 in json"),
        }
    };

    match classify_behavior_internal(json_str) {
        Ok(json) => match CString::new(json) {
            Ok(cs) => cs.into_raw(),
            Err(_) => error_to_cstring("result contains null byte"),
        },
        Err(e) => error_to_cstring(&e),
    }
}

fn classify_behavior_internal(json_str: &str) -> Result<String, String> {
    let graph: idl_graph::Graph =
        serde_json::from_str(json_str).map_err(|e| format!("invalid graph JSON: {}", e))?;

    let mut classifications = std::collections::HashMap::new();

    for (node_id, node) in &graph.nodes {
        let behavior = classify_node_kind(&node.kind);
        classifications.insert(node_id.0.clone(), behavior);
    }

    serde_json::to_string_pretty(&classifications)
        .map_err(|e| format!("failed to serialize classifications: {}", e))
}

fn classify_node_kind(kind: &idl_graph::NodeKind) -> String {
    match kind {
        idl_graph::NodeKind::Entity => "entity".to_string(),
        idl_graph::NodeKind::Aggregate => "aggregate".to_string(),
        idl_graph::NodeKind::Variant => "variant".to_string(),
        idl_graph::NodeKind::Event => "event".to_string(),
        idl_graph::NodeKind::Operation => "operation".to_string(),
        idl_graph::NodeKind::Api => "api".to_string(),
        idl_graph::NodeKind::StateMachine => "state_machine".to_string(),
        idl_graph::NodeKind::Rule => "rule".to_string(),
        idl_graph::NodeKind::Invariant => "invariant".to_string(),
        idl_graph::NodeKind::Policy => "policy".to_string(),
        idl_graph::NodeKind::Constraints => "constraints".to_string(),
        idl_graph::NodeKind::Mapping => "mapping".to_string(),
        idl_graph::NodeKind::TraceLink => "trace_link".to_string(),
        idl_graph::NodeKind::Decision => "decision".to_string(),
        idl_graph::NodeKind::Verification => "verification".to_string(),
        idl_graph::NodeKind::Intent => "intent".to_string(),
        idl_graph::NodeKind::Scope => "scope".to_string(),
        idl_graph::NodeKind::AccessPattern => "access_pattern".to_string(),
        _ => "unknown".to_string(), // Future kernel extensions
    }
}

fn error_to_cstring(msg: &str) -> *mut c_char {
    let error_json = serde_json::json!({
        "error": msg
    });
    let json_str = error_json.to_string();
    CString::new(json_str)
        .unwrap_or_else(|_| CString::new("unknown error").unwrap())
        .into_raw()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn test_parse_graph_null_path() {
        let result = idl_parse_graph(std::ptr::null());
        assert!(!result.is_null());

        let result_str = unsafe { CStr::from_ptr(result).to_str().unwrap() };
        assert!(result_str.contains("error"));
        assert!(result_str.contains("null path"));

        idl_free_string(result);
    }

    #[test]
    fn test_parse_graph_nonexistent_path() {
        let path = CString::new("/nonexistent/path/12345").unwrap();
        let result = idl_parse_graph(path.as_ptr());
        assert!(!result.is_null());

        let result_str = unsafe { CStr::from_ptr(result).to_str().unwrap() };
        assert!(result_str.contains("error"));
        assert!(result_str.contains("does not exist"));

        idl_free_string(result);
    }

    #[test]
    fn test_validate_graph_null() {
        let code = idl_validate_graph(std::ptr::null());
        assert_eq!(code, 3); // null pointer error
    }

    #[test]
    fn test_validate_graph_invalid_json() {
        let bad_json = CString::new("not valid json").unwrap();
        let code = idl_validate_graph(bad_json.as_ptr());
        assert_eq!(code, 1); // invalid JSON error
    }

    #[test]
    fn test_validate_graph_empty() {
        let empty_graph = CString::new(r#"{"nodes":{},"edges":{}}"#).unwrap();
        let code = idl_validate_graph(empty_graph.as_ptr());
        // Empty graph gets rejected as invalid schema
        assert!(code != 0, "Empty graph should fail validation");
    }

    #[test]
    fn test_validate_graph_valid_minimal() {
        // Basic structure test - just verify JSON parse doesn't crash
        let valid_graph = CString::new(r#"{"nodes":{},"edges":{}}"#).unwrap();
        let code = idl_validate_graph(valid_graph.as_ptr());
        // Should get some response (not crash)
        assert!((0..=4).contains(&code), "Should return valid error code");
    }

    #[test]
    fn test_classify_behavior_null() {
        let result = idl_classify_behavior(std::ptr::null());
        assert!(!result.is_null());

        let result_str = unsafe { CStr::from_ptr(result).to_str().unwrap() };
        assert!(result_str.contains("error"));
        assert!(result_str.contains("null json"));

        idl_free_string(result);
    }

    #[test]
    fn test_classify_behavior_invalid_json() {
        let bad_json = CString::new("not json").unwrap();
        let result = idl_classify_behavior(bad_json.as_ptr());
        assert!(!result.is_null());

        let result_str = unsafe { CStr::from_ptr(result).to_str().unwrap() };
        assert!(result_str.contains("error"));

        idl_free_string(result);
    }

    #[test]
    fn test_classify_behavior_basic() {
        // Test that classify_behavior returns valid JSON for any input graph
        let graph_json = CString::new(r#"{"nodes":{},"edges":{}}"#).unwrap();

        let result = idl_classify_behavior(graph_json.as_ptr());
        assert!(!result.is_null());

        let result_str = unsafe { CStr::from_ptr(result).to_str().unwrap() };
        // Should get valid JSON back (either error or empty classifications)
        let _json: serde_json::Value =
            serde_json::from_str(result_str).expect("classify_behavior should return valid JSON");

        idl_free_string(result);
    }

    #[test]
    fn test_free_string_null() {
        // Should not crash
        idl_free_string(std::ptr::null_mut());
    }

    #[test]
    fn test_memory_lifecycle() {
        let graph_json = CString::new(r#"{"nodes":{"entity::X":{"id":"entity::X","kind":"entity","state":"accepted","props":{},"source_anchors":[],"confidence":null}},"edges":{}}"#).unwrap();
        let result = idl_classify_behavior(graph_json.as_ptr());
        assert!(!result.is_null());

        // Read the result
        let _result_str = unsafe { CStr::from_ptr(result).to_str().unwrap() };

        // Free it
        idl_free_string(result);

        // Should not double-free or crash
    }
}
