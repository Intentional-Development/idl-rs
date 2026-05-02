//! Extension namespace `dto` — Wave 12 (Direction C, RFC dto-node-kind).
//!
//! DTOs are projections of a base `entity` for serialization purposes
//! (request/response bodies in OpenAPI, Conduit Java DTOs, etc.). They
//! live under `extensions.dto.definitions[]` and are referenced from
//! `operation.props.accepts.dto` / `operation.props.returns.dto`.
//!
//! Wave 15 second pass: `kind` discriminator enables enum-only, map-only,
//! unit, and object-projection DTOs. Backward compatible: absent `kind`
//! defaults to `"object"`.
//!
//! Wave 16: Three new features per W16 CONSENSUS:
//!   1. `nullable: boolean` on extras properties (orthogonal to requiredness).
//!   2. `kind: "array-alias"` with `items` field for bare array schemas.
//!   3. `kind: "union"` with `variants` array and optional `discriminator`.
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

/// DTO kind discriminator. Determines the DTO shape and which fields are valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DtoKind {
    /// Entity-projection DTO. Projects from a `base` entity via pick/omit/extras.
    Object,
    /// Enum-only type. Declares a closed set of string literal values.
    Enum,
    /// Map-only type. Declares an object with dynamic keys and fixed value schema.
    Map,
    /// Unit/empty type. Declares an object with no properties.
    Unit,
    /// Array-alias type (Wave 16). Bare array schema with `items` field.
    ArrayAlias,
    /// Union type (Wave 16). Polymorphic schema with `variants` array.
    Union,
    /// Paginated type (Wave 18). API list response with envelope wrapper (data array + pagination metadata).
    Paginated,
}

impl Default for DtoKind {
    fn default() -> Self {
        DtoKind::Object
    }
}

/// Parsed DTO definition. Fields mirror the JSON shape under
/// `extensions.dto.definitions[]` (see `IDL/schemas/semantic-graph.schema.json`
/// `$defs/DtoDefinition`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DtoDefinition {
    pub id: String,
    #[serde(default, skip_serializing_if = "is_default_kind")]
    pub kind: DtoKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    pub state: String,
    pub created_by: String,
    // Enum-kind fields
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_type: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub nullable: bool,
    // Array-alias fields (Wave 16)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<String>,
    // Union fields (Wave 16)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variants: Option<Vec<DtoVariant>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discriminator: Option<DtoDiscriminator>,
    // Paginated fields (Wave 18)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor_field: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_more_field: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_field: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta_fields: Option<BTreeMap<String, String>>,
    // Object-kind fields
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
    // Common fields
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_anchors: Vec<SourceAnchorDoc>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decision_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<ConfidenceDoc>,
}

fn is_default_kind(k: &DtoKind) -> bool {
    *k == DtoKind::Object
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
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub nullable: bool,
}

/// One variant in a union DTO (Wave 16).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DtoVariant {
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub ty: Option<String>,
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    pub ref_: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub array: bool,
}

/// Discriminator for a union DTO (Wave 16).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DtoDiscriminator {
    pub property: String,
    pub mapping: BTreeMap<String, String>,
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

        // 2. Kind-specific validation.
        match dto.kind {
            DtoKind::Object => {
                // Object-kind requires base.
                let Some(base_ref) = &dto.base else {
                    out.push(DtoViolation::new(
                        "dto-object-requires-base",
                        dto,
                        "kind: \"object\" requires base field".to_string(),
                    ));
                    continue;
                };

                // base must resolve to an existing entity.
                let Some(base_attrs) = entity_attrs.get(base_ref) else {
                    out.push(DtoViolation::new(
                        "dto-base-resolves",
                        dto,
                        format!("base {:?} does not resolve to an entity node", base_ref),
                    ));
                    continue;
                };

                // pick/omit mutual exclusion (defensive — schema also enforces).
                if dto.pick.is_some() && dto.omit.is_some() {
                    out.push(DtoViolation::new(
                        "dto-pick-omit-exclusive",
                        dto,
                        "pick and omit are mutually exclusive".to_string(),
                    ));
                }

                // wrapper DTO constraints.
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

                // wraps must resolve to a known DTO.
                if let Some(wraps_ref) = &dto.wraps {
                    if !seen_ids.contains(wraps_ref.as_str()) && !dtos.iter().any(|d| d.id == *wraps_ref) {
                        out.push(DtoViolation::new(
                            "dto-wrapper-wraps-resolves",
                            dto,
                            format!("wraps {:?} does not resolve to a known DTO id", wraps_ref),
                        ));
                    }
                }

                // pick/omit must be subsets of base attributes.
                if let Some(pick) = &dto.pick {
                    for name in pick {
                        if !base_attrs.contains(name) {
                            out.push(DtoViolation::new(
                                "dto-pick-subset",
                                dto,
                                format!(
                                    "pick field {:?} not present on base entity {} (available: {:?})",
                                    name,
                                    base_ref,
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
                                    name, base_ref
                                ),
                            ));
                        }
                    }
                }

                // required ⊆ projected set.
                let projected = compute_projected_fields(dto, base_attrs);
                for req in &dto.required {
                    if !projected.contains(req.as_str()) {
                        out.push(DtoViolation::new(
                            "dto-required-projected",
                            dto,
                            format!(
                                "required field {:?} not in projected set {:?}",
                                req,
                                projected.iter().collect::<Vec<_>>()
                            ),
                        ));
                    }
                }
            }
            DtoKind::Enum => {
                // Enum-kind requires values.
                if dto.values.is_none() || dto.values.as_ref().map_or(true, |v| v.is_empty()) {
                    out.push(DtoViolation::new(
                        "dto-enum-requires-values",
                        dto,
                        "kind: \"enum\" requires values with ≥1 item".to_string(),
                    ));
                }
                // Enum-kind forbids projection fields.
                if dto.base.is_some()
                    || dto.pick.is_some()
                    || dto.omit.is_some()
                    || !dto.extras.is_empty()
                    || dto.wrapper
                    || dto.wraps.is_some()
                {
                    out.push(DtoViolation::new(
                        "dto-enum-no-projection",
                        dto,
                        "kind: \"enum\" forbids base, pick, omit, extras, wrapper, wraps".to_string(),
                    ));
                }
            }
            DtoKind::Map => {
                // Map-kind requires value_type.
                if dto.value_type.is_none() {
                    out.push(DtoViolation::new(
                        "dto-map-requires-value-type",
                        dto,
                        "kind: \"map\" requires value_type".to_string(),
                    ));
                }
                // Map-kind forbids projection fields.
                if dto.base.is_some()
                    || dto.pick.is_some()
                    || dto.omit.is_some()
                    || !dto.extras.is_empty()
                    || dto.wrapper
                    || dto.wraps.is_some()
                {
                    out.push(DtoViolation::new(
                        "dto-map-no-projection",
                        dto,
                        "kind: \"map\" forbids base, pick, omit, extras, wrapper, wraps".to_string(),
                    ));
                }
                // value_type must resolve to known DTO id or valid primitive.
                if let Some(vt) = &dto.value_type {
                    let is_primitive = matches!(vt.as_str(), "string" | "integer" | "number" | "boolean");
                    let is_dto = vt.starts_with("dto:") && (seen_ids.contains(vt.as_str()) || dtos.iter().any(|d| d.id == *vt));
                    if !is_primitive && !is_dto {
                        out.push(DtoViolation::new(
                            "dto-map-value-type-resolves",
                            dto,
                            format!("value_type {:?} must be a valid primitive or known DTO id", vt),
                        ));
                    }
                }
            }
            DtoKind::Unit => {
                // Unit-kind forbids all projection and schema fields.
                if dto.base.is_some()
                    || dto.pick.is_some()
                    || dto.omit.is_some()
                    || !dto.extras.is_empty()
                    || dto.wrapper
                    || dto.wraps.is_some()
                    || dto.values.is_some()
                    || dto.value_type.is_some()
                {
                    out.push(DtoViolation::new(
                        "dto-unit-no-fields",
                        dto,
                        "kind: \"unit\" forbids base, pick, omit, extras, wrapper, wraps, values, value_type".to_string(),
                    ));
                }
            }
            DtoKind::ArrayAlias => {
                // Array-alias requires items.
                if dto.items.is_none() {
                    out.push(DtoViolation::new(
                        "dto-array-alias-requires-items",
                        dto,
                        "kind: \"array-alias\" requires items field".to_string(),
                    ));
                }
                // Array-alias forbids projection fields.
                if dto.base.is_some()
                    || dto.pick.is_some()
                    || dto.omit.is_some()
                    || !dto.extras.is_empty()
                    || dto.wrapper
                    || dto.wraps.is_some()
                    || dto.values.is_some()
                    || dto.value_type.is_some()
                {
                    out.push(DtoViolation::new(
                        "dto-array-alias-no-projection",
                        dto,
                        "kind: \"array-alias\" forbids base, pick, omit, extras, wrapper, wraps, values, value_type".to_string(),
                    ));
                }
                // items must resolve to known DTO id or valid primitive.
                if let Some(items_ref) = &dto.items {
                    let is_primitive = matches!(items_ref.as_str(), "string" | "integer" | "number" | "boolean");
                    let is_dto = items_ref.starts_with("dto:") && (seen_ids.contains(items_ref.as_str()) || dtos.iter().any(|d| d.id == *items_ref));
                    if !is_primitive && !is_dto {
                        out.push(DtoViolation::new(
                            "dto-array-alias-items-resolves",
                            dto,
                            format!("items {:?} must be a valid primitive or known DTO id", items_ref),
                        ));
                    }
                }
            }
            DtoKind::Union => {
                // Union requires variants with ≥2 items.
                if dto.variants.is_none() || dto.variants.as_ref().map_or(true, |v| v.len() < 2) {
                    out.push(DtoViolation::new(
                        "dto-union-requires-variants",
                        dto,
                        "kind: \"union\" requires variants with ≥2 items".to_string(),
                    ));
                }
                // Union forbids projection fields.
                if dto.base.is_some()
                    || dto.pick.is_some()
                    || dto.omit.is_some()
                    || !dto.extras.is_empty()
                    || dto.wrapper
                    || dto.wraps.is_some()
                    || dto.values.is_some()
                    || dto.value_type.is_some()
                    || dto.items.is_some()
                {
                    out.push(DtoViolation::new(
                        "dto-union-no-projection",
                        dto,
                        "kind: \"union\" forbids base, pick, omit, extras, wrapper, wraps, values, value_type, items".to_string(),
                    ));
                }
                // Each variant with ref must resolve to known DTO id.
                if let Some(variants) = &dto.variants {
                    for (i, var) in variants.iter().enumerate() {
                        if let Some(ref_) = &var.ref_ {
                            if !seen_ids.contains(ref_.as_str()) && !dtos.iter().any(|d| d.id == *ref_) {
                                out.push(DtoViolation::new(
                                    "dto-union-variant-resolves",
                                    dto,
                                    format!("variants[{}].ref {:?} does not resolve to a known DTO id", i, ref_),
                                ));
                            }
                        }
                    }
                }
                // discriminator only valid for union kind (checked globally below).
            }
            DtoKind::Paginated => {
                // Paginated requires items.
                if dto.items.is_none() {
                    out.push(DtoViolation::new(
                        "dto-paginated-requires-items",
                        dto,
                        "kind: \"paginated\" requires items field".to_string(),
                    ));
                }
                // Paginated forbids projection fields.
                if dto.base.is_some()
                    || dto.pick.is_some()
                    || dto.omit.is_some()
                    || !dto.extras.is_empty()
                    || dto.wrapper
                    || dto.wraps.is_some()
                    || dto.values.is_some()
                    || dto.value_type.is_some()
                    || dto.variants.is_some()
                    || dto.discriminator.is_some()
                {
                    out.push(DtoViolation::new(
                        "dto-paginated-no-projection",
                        dto,
                        "kind: \"paginated\" forbids base, pick, omit, extras, wrapper, wraps, values, value_type, variants, discriminator".to_string(),
                    ));
                }
                // items must resolve to known DTO id or valid primitive.
                if let Some(items_ref) = &dto.items {
                    let is_primitive = matches!(items_ref.as_str(), "string" | "integer" | "number" | "boolean");
                    let is_dto = items_ref.starts_with("dto:") && (seen_ids.contains(items_ref.as_str()) || dtos.iter().any(|d| d.id == *items_ref));
                    if !is_primitive && !is_dto {
                        out.push(DtoViolation::new(
                            "dto-paginated-items-resolves",
                            dto,
                            format!("items {:?} must be a valid primitive or known DTO id", items_ref),
                        ));
                    }
                }
            }
        }

        // discriminator field only allowed when kind=union.
        if dto.discriminator.is_some() && dto.kind != DtoKind::Union {
            out.push(DtoViolation::new(
                "dto-discriminator-requires-union",
                dto,
                "discriminator only valid on kind: \"union\"".to_string(),
            ));
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
/// Used for validation to ensure `required ⊆ projected`.
pub fn compute_projected_fields(
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

/// Legacy alias for backward compatibility.
#[deprecated(since = "0.1.5", note = "Use `compute_projected_fields` instead")]
pub fn project_field_set(
    dto: &DtoDefinition,
    base_attrs: &BTreeSet<String>,
) -> BTreeSet<String> {
    compute_projected_fields(dto, base_attrs)
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

    #[test]
    fn test_enum_kind_requires_values() {
        let graph = serde_json::from_value(json!({
            "version": "0.1.5",
            "nodes": [],
            "edges": [],
            "extensions": {
                "dto": {
                    "definitions": [
                        {
                            "id": "dto:DeviceType",
                            "kind": "enum",
                            "state": "proposed",
                            "created_by": "ai"
                        }
                    ]
                }
            }
        })).unwrap();

        let violations = validate_dtos(&graph);
        assert!(violations.iter().any(|v| v.rule == "dto-enum-requires-values"));
    }

    #[test]
    fn test_enum_kind_forbids_projection() {
        let mut graph = minimal_graph_with_user_entity();
        graph.extensions = Some(json!({
            "dto": {
                "definitions": [
                    {
                        "id": "dto:DeviceType",
                        "kind": "enum",
                        "state": "proposed",
                        "created_by": "ai",
                        "values": ["mobile", "desktop"],
                        "base": "entity:user"
                    }
                ]
            }
        }));

        let violations = validate_dtos(&graph);
        assert!(violations.iter().any(|v| v.rule == "dto-enum-no-projection"));
    }

    #[test]
    fn test_valid_enum_kind() {
        let graph = serde_json::from_value(json!({
            "version": "0.1.5",
            "nodes": [],
            "edges": [],
            "extensions": {
                "dto": {
                    "definitions": [
                        {
                            "id": "dto:DeviceType",
                            "kind": "enum",
                            "state": "proposed",
                            "created_by": "ai",
                            "values": ["mobile", "desktop", "web"]
                        }
                    ]
                }
            }
        })).unwrap();

        let violations = validate_dtos(&graph);
        let enum_violations: Vec<_> = violations.iter()
            .filter(|v| v.dto_id == "dto:DeviceType")
            .collect();
        assert!(enum_violations.is_empty(), "Expected no violations, got: {:?}", enum_violations);
    }

    #[test]
    fn test_map_kind_requires_value_type() {
        let graph = serde_json::from_value(json!({
            "version": "0.1.5",
            "nodes": [],
            "edges": [],
            "extensions": {
                "dto": {
                    "definitions": [
                        {
                            "id": "dto:PrepareUploadResponse",
                            "kind": "map",
                            "state": "proposed",
                            "created_by": "ai"
                        }
                    ]
                }
            }
        })).unwrap();

        let violations = validate_dtos(&graph);
        assert!(violations.iter().any(|v| v.rule == "dto-map-requires-value-type"));
    }

    #[test]
    fn test_map_kind_forbids_projection() {
        let mut graph = minimal_graph_with_user_entity();
        graph.extensions = Some(json!({
            "dto": {
                "definitions": [
                    {
                        "id": "dto:PrepareUploadResponse",
                        "kind": "map",
                        "state": "proposed",
                        "created_by": "ai",
                        "value_type": "string",
                        "base": "entity:user"
                    }
                ]
            }
        }));

        let violations = validate_dtos(&graph);
        assert!(violations.iter().any(|v| v.rule == "dto-map-no-projection"));
    }

    #[test]
    fn test_valid_map_kind() {
        let graph = serde_json::from_value(json!({
            "version": "0.1.5",
            "nodes": [],
            "edges": [],
            "extensions": {
                "dto": {
                    "definitions": [
                        {
                            "id": "dto:PrepareUploadResponse",
                            "kind": "map",
                            "state": "proposed",
                            "created_by": "ai",
                            "value_type": "string"
                        }
                    ]
                }
            }
        })).unwrap();

        let violations = validate_dtos(&graph);
        let map_violations: Vec<_> = violations.iter()
            .filter(|v| v.dto_id == "dto:PrepareUploadResponse")
            .collect();
        assert!(map_violations.is_empty(), "Expected no violations, got: {:?}", map_violations);
    }

    #[test]
    fn test_unit_kind_forbids_all_fields() {
        let graph = serde_json::from_value(json!({
            "version": "0.1.5",
            "nodes": [],
            "edges": [],
            "extensions": {
                "dto": {
                    "definitions": [
                        {
                            "id": "dto:EmptyPayload",
                            "kind": "unit",
                            "state": "proposed",
                            "created_by": "ai",
                            "values": ["should", "not", "be", "here"]
                        }
                    ]
                }
            }
        })).unwrap();

        let violations = validate_dtos(&graph);
        assert!(violations.iter().any(|v| v.rule == "dto-unit-no-fields"));
    }

    #[test]
    fn test_valid_unit_kind() {
        let graph = serde_json::from_value(json!({
            "version": "0.1.5",
            "nodes": [],
            "edges": [],
            "extensions": {
                "dto": {
                    "definitions": [
                        {
                            "id": "dto:EmptyPayload",
                            "kind": "unit",
                            "state": "proposed",
                            "created_by": "ai"
                        }
                    ]
                }
            }
        })).unwrap();

        let violations = validate_dtos(&graph);
        let unit_violations: Vec<_> = violations.iter()
            .filter(|v| v.dto_id == "dto:EmptyPayload")
            .collect();
        assert!(unit_violations.is_empty(), "Expected no violations, got: {:?}", unit_violations);
    }

    #[test]
    fn test_backward_compat_object_kind_implicit() {
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
                    }
                ]
            }
        }));

        let dtos = parse_dtos(&graph).unwrap();
        assert_eq!(dtos[0].kind, DtoKind::Object, "Absent kind should default to Object");
        
        let violations = validate_dtos(&graph);
        let dto_violations: Vec<_> = violations.iter()
            .filter(|v| v.dto_id == "dto:User")
            .collect();
        assert!(dto_violations.is_empty(), "Expected no violations for backward-compat DTO, got: {:?}", dto_violations);
    }

    // Wave 18: Paginated kind tests
    #[test]
    fn test_paginated_stripe_style() {
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
                        "id": "dto:ChargeList",
                        "kind": "paginated",
                        "state": "proposed",
                        "created_by": "ai",
                        "items": "dto:User",
                        "has_more_field": "has_more",
                        "meta_fields": {
                            "url": "string",
                            "object": "string"
                        }
                    }
                ]
            }
        }));

        let dtos = parse_dtos(&graph).unwrap();
        let paginated = dtos.iter().find(|d| d.id == "dto:ChargeList").unwrap();
        assert_eq!(paginated.kind, DtoKind::Paginated);
        assert_eq!(paginated.items, Some("dto:User".to_string()));
        assert_eq!(paginated.has_more_field, Some("has_more".to_string()));
        assert!(paginated.meta_fields.is_some());

        let violations = validate_dtos(&graph);
        let pag_violations: Vec<_> = violations.iter()
            .filter(|v| v.dto_id == "dto:ChargeList")
            .collect();
        assert!(pag_violations.is_empty(), "Expected no violations for Stripe-style paginated DTO, got: {:?}", pag_violations);
    }

    #[test]
    fn test_paginated_firefly_style() {
        let mut graph = minimal_graph_with_user_entity();
        graph.extensions = Some(json!({
            "dto": {
                "definitions": [
                    {
                        "id": "dto:AccountRead",
                        "base": "entity:user",
                        "state": "proposed",
                        "created_by": "ai"
                    },
                    {
                        "id": "dto:AccountArray",
                        "kind": "paginated",
                        "state": "proposed",
                        "created_by": "ai",
                        "items": "dto:AccountRead",
                        "total_field": "meta.pagination.total",
                        "meta_fields": {
                            "meta": "object",
                            "links": "object"
                        }
                    }
                ]
            }
        }));

        let dtos = parse_dtos(&graph).unwrap();
        let paginated = dtos.iter().find(|d| d.id == "dto:AccountArray").unwrap();
        assert_eq!(paginated.kind, DtoKind::Paginated);
        assert_eq!(paginated.items, Some("dto:AccountRead".to_string()));
        assert_eq!(paginated.total_field, Some("meta.pagination.total".to_string()));

        let violations = validate_dtos(&graph);
        let pag_violations: Vec<_> = violations.iter()
            .filter(|v| v.dto_id == "dto:AccountArray")
            .collect();
        assert!(pag_violations.is_empty(), "Expected no violations for firefly-style paginated DTO, got: {:?}", pag_violations);
    }

    #[test]
    fn test_paginated_minimal() {
        let graph = serde_json::from_value(json!({
            "version": "0.1.7",
            "nodes": [],
            "edges": [],
            "extensions": {
                "dto": {
                    "definitions": [
                        {
                            "id": "dto:SimpleList",
                            "kind": "paginated",
                            "state": "proposed",
                            "created_by": "ai",
                            "items": "string"
                        }
                    ]
                }
            }
        })).unwrap();

        let dtos = parse_dtos(&graph).unwrap();
        assert_eq!(dtos[0].kind, DtoKind::Paginated);
        assert_eq!(dtos[0].items, Some("string".to_string()));

        let violations = validate_dtos(&graph);
        let pag_violations: Vec<_> = violations.iter()
            .filter(|v| v.dto_id == "dto:SimpleList")
            .collect();
        assert!(pag_violations.is_empty(), "Expected no violations for minimal paginated DTO, got: {:?}", pag_violations);
    }

    #[test]
    fn test_paginated_full_form() {
        let mut graph = minimal_graph_with_user_entity();
        graph.extensions = Some(json!({
            "dto": {
                "definitions": [
                    {
                        "id": "dto:Item",
                        "base": "entity:user",
                        "state": "proposed",
                        "created_by": "ai"
                    },
                    {
                        "id": "dto:FullList",
                        "kind": "paginated",
                        "state": "proposed",
                        "created_by": "ai",
                        "items": "dto:Item",
                        "cursor_field": "starting_after",
                        "has_more_field": "has_more",
                        "total_field": "total",
                        "meta_fields": {
                            "url": "string",
                            "object": "string",
                            "count": "integer"
                        }
                    }
                ]
            }
        }));

        let dtos = parse_dtos(&graph).unwrap();
        let paginated = dtos.iter().find(|d| d.id == "dto:FullList").unwrap();
        assert_eq!(paginated.kind, DtoKind::Paginated);
        assert_eq!(paginated.cursor_field, Some("starting_after".to_string()));
        assert_eq!(paginated.has_more_field, Some("has_more".to_string()));
        assert_eq!(paginated.total_field, Some("total".to_string()));
        assert!(paginated.meta_fields.as_ref().unwrap().contains_key("url"));

        let violations = validate_dtos(&graph);
        let pag_violations: Vec<_> = violations.iter()
            .filter(|v| v.dto_id == "dto:FullList")
            .collect();
        assert!(pag_violations.is_empty(), "Expected no violations for full-form paginated DTO, got: {:?}", pag_violations);
    }

    #[test]
    fn test_paginated_requires_items() {
        let graph = serde_json::from_value(json!({
            "version": "0.1.7",
            "nodes": [],
            "edges": [],
            "extensions": {
                "dto": {
                    "definitions": [
                        {
                            "id": "dto:BrokenList",
                            "kind": "paginated",
                            "state": "proposed",
                            "created_by": "ai"
                        }
                    ]
                }
            }
        })).unwrap();

        let violations = validate_dtos(&graph);
        assert!(violations.iter().any(|v| v.rule == "dto-paginated-requires-items"));
    }

    #[test]
    fn test_paginated_forbids_projection_fields() {
        let mut graph = minimal_graph_with_user_entity();
        graph.extensions = Some(json!({
            "dto": {
                "definitions": [
                    {
                        "id": "dto:BrokenList",
                        "kind": "paginated",
                        "state": "proposed",
                        "created_by": "ai",
                        "items": "string",
                        "base": "entity:user",
                        "pick": ["email"]
                    }
                ]
            }
        }));

        let violations = validate_dtos(&graph);
        assert!(violations.iter().any(|v| v.rule == "dto-paginated-no-projection"));
    }
}
