//! Extension namespace `dto` — Wave 12 (Direction C, RFC dto-node-kind).
//!
//! DTOs are projections of a base `entity` for serialization purposes
//! (request/response bodies in OpenAPI, Conduit Java DTOs, etc.). They
//! live under `extensions.dto.definitions[]` and are referenced from
//! `operation.props.accepts.dto` / `operation.props.returns.dto`.
//!
//! This module owns:
//!   1. The `DtoDefinition` shape (parsed from the JSON graph document).
//!   2. `validate_dtos(graph) -> Vec<DtoViolation>` — semantic validation
//!      against the in-doc entity surface (id pattern, base resolution,
//!      pick/omit subset, mutual exclusion, required ⊆ projected fields,
//!      provenance for accepted state, accepts/returns refs resolved).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::doc::{ConfidenceDoc, GraphDoc, SourceAnchorDoc};

/// Parsed DTO definition. Fields mirror the JSON shape under
/// `extensions.dto.definitions[]` (see `IDL/schemas/semantic-graph.schema.json`
/// `$defs/DtoDefinition`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DtoDefinition {
    pub id: String,
    pub base: String,
    pub state: String,
    pub created_by: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub wrapper: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wraps: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pick: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub omit: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extras: BTreeMap<String, DtoExtra>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_anchors: Vec<SourceAnchorDoc>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decision_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<ConfidenceDoc>,
}

/// One entry under `DtoDefinition.extras`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DtoExtra {
    #[serde(rename = "type")]
    pub ty: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub optional: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
}

/// Validation finding for a DTO definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DtoViolation {
    pub rule: String,
    pub dto_id: String,
    pub message: String,
    pub anchor_uris: Vec<String>,
}

impl DtoViolation {
    fn new(rule: &str, dto: &DtoDefinition, msg: impl Into<String>) -> Self {
        Self {
            rule: rule.to_string(),
            dto_id: dto.id.clone(),
            message: msg.into(),
            anchor_uris: dto.source_anchors.iter().map(|a| a.uri.clone()).collect(),
        }
    }
}

/// Parse the `extensions.dto.definitions[]` array out of a [`GraphDoc`].
/// Returns an empty vec if the namespace is absent.
pub fn parse_dtos(graph: &GraphDoc) -> Result<Vec<DtoDefinition>, String> {
    let Some(ext) = &graph.extensions else { return Ok(Vec::new()); };
    let Some(dto_ns) = ext.get("dto") else { return Ok(Vec::new()); };
    let Some(defs) = dto_ns.get("definitions") else { return Ok(Vec::new()); };
    let Some(arr) = defs.as_array() else {
        return Err("extensions.dto.definitions must be an array".into());
    };
    let mut out = Vec::with_capacity(arr.len());
    for (i, raw) in arr.iter().enumerate() {
        let dto: DtoDefinition = serde_json::from_value(raw.clone())
            .map_err(|e| format!("extensions.dto.definitions[{i}]: {e}"))?;
        out.push(dto);
    }
    Ok(out)
}

/// Run semantic validation over the DTO namespace. Errors are reported
/// per-DTO with the DTO's source-anchor URIs surfaced for navigation.
pub fn validate_dtos(graph: &GraphDoc) -> Vec<DtoViolation> {
    let mut out = Vec::new();
    let dtos = match parse_dtos(graph) {
        Ok(d) => d,
        Err(e) => {
            out.push(DtoViolation {
                rule: "dto-parse".into(),
                dto_id: "<unparsed>".into(),
                message: e,
                anchor_uris: vec![],
            });
            return out;
        }
    };

    // Index entity attribute names for `base` resolution.
    let mut entity_attrs: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for n in graph.nodes_of_kind("entity") {
        let mut names: BTreeSet<String> = BTreeSet::new();
        if let Some(arr) = n.props.get("attributes").and_then(|v| v.as_array()) {
            for a in arr {
                if let Some(name) = a.get("name").and_then(|v| v.as_str()) {
                    names.insert(name.to_string());
                }
            }
        }
        entity_attrs.insert(n.id.clone(), names);
    }

    let mut seen_ids: BTreeSet<&str> = BTreeSet::new();
    for dto in &dtos {
        // 1. id pattern (defensive — schema also enforces).
        if !dto.id.starts_with("dto:") || dto.id.len() <= 4 {
            out.push(DtoViolation::new(
                "dto-id-shape",
                dto,
                format!("id {:?} must match `dto:<Name>`", dto.id),
            ));
        }
        if !seen_ids.insert(dto.id.as_str()) {
            out.push(DtoViolation::new(
                "dto-id-unique",
                dto,
                format!("duplicate DTO id {:?}", dto.id),
            ));
        }

        // 2. base must resolve to an existing entity.
        let base_attrs = match entity_attrs.get(&dto.base) {
            Some(s) => s,
            None => {
                out.push(DtoViolation::new(
                    "dto-base-resolves",
                    dto,
                    format!("base {:?} does not resolve to an entity node", dto.base),
                ));
                continue;
            }
        };

        // 3. pick/omit mutual exclusion (defensive — schema also enforces).
        if dto.pick.is_some() && dto.omit.is_some() {
            out.push(DtoViolation::new(
                "dto-pick-omit-exclusive",
                dto,
                "pick and omit are mutually exclusive".to_string(),
            ));
        }

        // 3a. wrapper DTO constraints.
        if dto.wrapper {
            if dto.wraps.is_none() {
                out.push(DtoViolation::new(
                    "dto-wrapper-requires-wraps",
                    dto,
                    "wrapper=true requires wraps field".to_string(),
                ));
            }
            if dto.pick.is_some() || dto.omit.is_some() || !dto.extras.is_empty() {
                out.push(DtoViolation::new(
                    "dto-wrapper-no-projection",
                    dto,
                    "wrapper DTOs cannot have pick, omit, or extras".to_string(),
                ));
            }
        }

        // 3b. wraps must resolve to a known DTO.
        if let Some(wraps_ref) = &dto.wraps {
            if !seen_ids.contains(wraps_ref.as_str()) && !dtos.iter().any(|d| d.id == *wraps_ref) {
                out.push(DtoViolation::new(
                    "dto-wrapper-wraps-resolves",
                    dto,
                    format!("wraps {:?} does not resolve to a known DTO id", wraps_ref),
                ));
            }
        }

        // 4. pick/omit must be subsets of base attributes (skip for wrappers).
        if let Some(pick) = &dto.pick {
            for name in pick {
                if !base_attrs.contains(name) {
                    out.push(DtoViolation::new(
                        "dto-pick-subset",
                        dto,
                        format!(
                            "pick field {:?} not present on base entity {} (available: {:?})",
                            name,
                            dto.base,
                            base_attrs.iter().collect::<Vec<_>>()
                        ),
                    ));
                }
            }
        }
        if let Some(omit) = &dto.omit {
            for name in omit {
                if !base_attrs.contains(name) {
                    out.push(DtoViolation::new(
                        "dto-omit-subset",
                        dto,
                        format!(
                            "omit field {:?} not present on base entity {}",
                            name, dto.base
                        ),
                    ));
                }
            }
        }

        // 5. required ⊆ projected ∪ extras (skip for wrappers).
        if !dto.wrapper {
            let projected = project_field_set(dto, base_attrs);
            for name in &dto.required {
                if !projected.contains(name) {
                    out.push(DtoViolation::new(
                        "dto-required-projected",
                        dto,
                        format!(
                            "required field {:?} is not in the projected set (pick/omit + extras)",
                            name
                        ),
                    ));
                }
            }
        }

        // 6. accepted-state needs anchors or decision_refs.
        if dto.state == "accepted"
            && dto.source_anchors.is_empty()
            && dto.decision_refs.is_empty()
        {
            out.push(DtoViolation::new(
                "dto-accepted-provenance",
                dto,
                "state=accepted requires source_anchors or decision_refs".to_string(),
            ));
        }
    }

    // 7. operation.props.accepts.dto / returns.dto must resolve.
    let known_dto_ids: BTreeSet<&str> = dtos.iter().map(|d| d.id.as_str()).collect();
    for op in graph.nodes_of_kind("operation") {
        for axis in ["accepts", "returns"] {
            if let Some(refv) = op
                .props
                .get(axis)
                .and_then(|v| v.get("dto"))
                .and_then(|v| v.as_str())
            {
                if !known_dto_ids.contains(refv) {
                    out.push(DtoViolation {
                        rule: format!("dto-{axis}-resolves"),
                        dto_id: refv.to_string(),
                        message: format!(
                            "operation {:?}.props.{axis}.dto = {:?} does not resolve to any extensions.dto.definitions[].id",
                            op.id, refv
                        ),
                        anchor_uris: op.source_anchors.iter().map(|a| a.uri.clone()).collect(),
                    });
                }
            }
        }
    }

    out
}

/// Compute the projected field set for emitter use: `(base ∖ omit) ∩ pick ∪ extras`.
pub fn project_field_set(
    dto: &DtoDefinition,
    base_attrs: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = match (&dto.pick, &dto.omit) {
        (Some(pick), _) => pick.iter().filter(|n| base_attrs.contains(*n)).cloned().collect(),
        (None, Some(omit)) => {
            let omit_set: BTreeSet<&String> = omit.iter().collect();
            base_attrs.iter().filter(|n| !omit_set.contains(n)).cloned().collect()
        }
        (None, None) => base_attrs.iter().cloned().collect(),
    };
    for k in dto.extras.keys() {
        out.insert(k.clone());
    }
    out
}

/// Convenience for the emitter: returns the ordered projected attribute
/// list (preserving entity order for picked/omitted fields, then extras
/// in insertion order).
pub fn projected_fields_ordered(
    dto: &DtoDefinition,
    base_attrs_ordered: &[Value],
) -> Vec<ProjectedField> {
    let mut out = Vec::new();
    let pick_set: Option<BTreeSet<&String>> = dto.pick.as_ref().map(|v| v.iter().collect());
    let omit_set: Option<BTreeSet<&String>> = dto.omit.as_ref().map(|v| v.iter().collect());
    for a in base_attrs_ordered {
        let Some(name) = a.get("name").and_then(|v| v.as_str()) else { continue };
        if let Some(p) = &pick_set {
            if !p.contains(&name.to_string()) {
                continue;
            }
        }
        if let Some(o) = &omit_set {
            if o.contains(&name.to_string()) {
                continue;
            }
        }
        let ty = a.get("type").and_then(|v| v.as_str()).unwrap_or("string").to_string();
        let nullable = a.get("nullable").and_then(|v| v.as_bool()).unwrap_or(false);
        out.push(ProjectedField {
            name: name.to_string(),
            ty,
            nullable,
            format: None,
            from_extras: false,
        });
    }
    for (k, v) in &dto.extras {
        out.push(ProjectedField {
            name: k.clone(),
            ty: v.ty.clone(),
            nullable: v.optional,
            format: v.format.clone(),
            from_extras: true,
        });
    }
    out
}

/// Field as resolved against the base entity (or extras).
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectedField {
    pub name: String,
    pub ty: String,
    pub nullable: bool,
    pub format: Option<String>,
    pub from_extras: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn minimal_graph_with_user_entity() -> GraphDoc {
        serde_json::from_value(json!({
            "version": "0.1.3",
            "nodes": [
                {
                    "id": "entity:user",
                    "kind": "entity",
                    "state": "accepted",
                    "created_by": "human",
                    "props": {
                        "name": "user",
                        "attributes": [
                            {"name": "email", "type": "string"},
                            {"name": "username", "type": "string"}
                        ]
                    },
                    "source_anchors": [{"uri": "test://user"}]
                }
            ],
            "edges": []
        }))
        .unwrap()
    }

    #[test]
    fn test_wrapper_dto_requires_wraps() {
        let mut graph = minimal_graph_with_user_entity();
        graph.extensions = Some(json!({
            "dto": {
                "definitions": [
                    {
                        "id": "dto:UserResponse",
                        "base": "entity:user",
                        "state": "proposed",
                        "created_by": "ai",
                        "wrapper": true
                    }
                ]
            }
        }));

        let violations = validate_dtos(&graph);
        assert!(violations.iter().any(|v| v.rule == "dto-wrapper-requires-wraps"));
    }

    #[test]
    fn test_wrapper_dto_cannot_have_pick_omit_extras() {
        let mut graph = minimal_graph_with_user_entity();
        graph.extensions = Some(json!({
            "dto": {
                "definitions": [
                    {
                        "id": "dto:User",
                        "base": "entity:user",
                        "state": "proposed",
                        "created_by": "ai",
                        "pick": ["email"]
                    },
                    {
                        "id": "dto:UserResponse",
                        "base": "entity:user",
                        "state": "proposed",
                        "created_by": "ai",
                        "wrapper": true,
                        "wraps": "dto:User",
                        "pick": ["email"]
                    }
                ]
            }
        }));

        let violations = validate_dtos(&graph);
        assert!(violations.iter().any(|v| v.rule == "dto-wrapper-no-projection"));
    }

    #[test]
    fn test_wrapper_dto_wraps_must_resolve() {
        let mut graph = minimal_graph_with_user_entity();
        graph.extensions = Some(json!({
            "dto": {
                "definitions": [
                    {
                        "id": "dto:UserResponse",
                        "base": "entity:user",
                        "state": "proposed",
                        "created_by": "ai",
                        "wrapper": true,
                        "wraps": "dto:NonExistent"
                    }
                ]
            }
        }));

        let violations = validate_dtos(&graph);
        assert!(violations.iter().any(|v| v.rule == "dto-wrapper-wraps-resolves"));
    }

    #[test]
    fn test_valid_wrapper_dto() {
        let mut graph = minimal_graph_with_user_entity();
        graph.extensions = Some(json!({
            "dto": {
                "definitions": [
                    {
                        "id": "dto:User",
                        "base": "entity:user",
                        "state": "proposed",
                        "created_by": "ai",
                        "pick": ["email", "username"]
                    },
                    {
                        "id": "dto:UserResponse",
                        "base": "entity:user",
                        "state": "proposed",
                        "created_by": "ai",
                        "wrapper": true,
                        "wraps": "dto:User"
                    }
                ]
            }
        }));

        let violations = validate_dtos(&graph);
        let wrapper_violations: Vec<_> = violations.iter()
            .filter(|v| v.dto_id == "dto:UserResponse")
            .collect();
        assert!(wrapper_violations.is_empty(), "Expected no violations, got: {:?}", wrapper_violations);
    }
}
