//! Kernel node and edge taxonomy.
//!
//! Frozen by the AI-Kernel Consensus dated 2026-05-01.
//! See `feedback/kernel-debate/CONSENSUS.md`. Do NOT add variants without
//! re-running the AI utility test (Q1/Q2/Q3) and a debate round.
//!
//! Both enums are `#[non_exhaustive]` so future kernel-extension constructs
//! land via debate rather than as a breaking change.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Kernel construct types. 21 variants (extended from 18 in v0.1.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Intent,
    Scope,
    Entity,
    Aggregate,
    Variant,
    Constraints,
    Event,
    Operation,
    StateMachine,
    Rule,
    Invariant,
    Policy,
    Api,
    AccessPattern,
    Mapping,
    TraceLink,
    Decision,
    Verification,
    ConsumerContract,
    Selector,
    LogEvent,
}

impl NodeKind {
    /// Stable canonical name used in IDL files and graph serialization.
    pub const fn canonical(self) -> &'static str {
        match self {
            NodeKind::Intent => "intent",
            NodeKind::Scope => "scope",
            NodeKind::Entity => "entity",
            NodeKind::Aggregate => "aggregate",
            NodeKind::Variant => "variant",
            NodeKind::Constraints => "constraints",
            NodeKind::Event => "event",
            NodeKind::Operation => "operation",
            NodeKind::StateMachine => "state_machine",
            NodeKind::Rule => "rule",
            NodeKind::Invariant => "invariant",
            NodeKind::Policy => "policy",
            NodeKind::Api => "api",
            NodeKind::AccessPattern => "access_pattern",
            NodeKind::Mapping => "mapping",
            NodeKind::TraceLink => "trace_link",
            NodeKind::Decision => "decision",
            NodeKind::Verification => "verification",
            NodeKind::ConsumerContract => "consumer_contract",
            NodeKind::Selector => "selector",
            NodeKind::LogEvent => "log_event",
        }
    }

    /// All 21 kernel variants, in canonical order.
    pub const ALL: [NodeKind; 21] = [
        NodeKind::Intent,
        NodeKind::Scope,
        NodeKind::Entity,
        NodeKind::Aggregate,
        NodeKind::Variant,
        NodeKind::Constraints,
        NodeKind::Event,
        NodeKind::Operation,
        NodeKind::StateMachine,
        NodeKind::Rule,
        NodeKind::Invariant,
        NodeKind::Policy,
        NodeKind::Api,
        NodeKind::AccessPattern,
        NodeKind::Mapping,
        NodeKind::TraceLink,
        NodeKind::Decision,
        NodeKind::Verification,
        NodeKind::ConsumerContract,
        NodeKind::Selector,
        NodeKind::LogEvent,
    ];

    /// Every kernel-listed `NodeKind` is in the kernel by construction.
    /// Extension constructs will live in a separate enum / string-tagged set.
    pub const fn is_kernel(&self) -> bool {
        true
    }
}

impl fmt::Display for NodeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.canonical())
    }
}

/// Returned by [`NodeKind::from_str`] / [`EdgeKind::from_str`] when the input
/// is not a recognized kernel canonical name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseKindError {
    pub input: String,
}

impl fmt::Display for ParseKindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown kernel kind: {:?}", self.input)
    }
}

impl std::error::Error for ParseKindError {}

impl FromStr for NodeKind {
    type Err = ParseKindError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "intent" => NodeKind::Intent,
            "scope" => NodeKind::Scope,
            "entity" => NodeKind::Entity,
            "aggregate" => NodeKind::Aggregate,
            "variant" => NodeKind::Variant,
            "constraints" => NodeKind::Constraints,
            "event" => NodeKind::Event,
            "operation" => NodeKind::Operation,
            "state_machine" => NodeKind::StateMachine,
            "rule" => NodeKind::Rule,
            "invariant" => NodeKind::Invariant,
            "policy" => NodeKind::Policy,
            "api" => NodeKind::Api,
            "access_pattern" => NodeKind::AccessPattern,
            "mapping" => NodeKind::Mapping,
            "trace_link" => NodeKind::TraceLink,
            "decision" => NodeKind::Decision,
            "verification" => NodeKind::Verification,
            "consumer_contract" => NodeKind::ConsumerContract,
            "selector" => NodeKind::Selector,
            "log_event" => NodeKind::LogEvent,
            other => {
                return Err(ParseKindError {
                    input: other.to_string(),
                })
            }
        })
    }
}

/// Kernel edge types. Each variant is derived from a kernel-construct relationship
/// that must be mechanically checkable (Q2). Extended from 18 to 21 variants in v0.1.10.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// Artifact realizes / fulfills an `intent`.       (any → Intent)
    Realizes,
    /// `verification` proves a claim about another node.  (Verification → any)
    Verifies,
    /// `event` triggers an `operation` (or state transition). (Event → Operation)
    Triggers,
    /// `operation` emits an `event`.                    (Operation → Event)
    Emits,
    /// `operation` handles / consumes an `event`.       (Operation → Event)
    Handles,
    /// `policy` / `rule` / `invariant` / `constraints` constrain a target.
    Constrains,
    /// Generic typed provenance edge (`trace_link`).    (any → any)
    TracesTo,
    /// `mapping` links an IDL node to a code artifact.  (NodeKind → code-ref)
    ExtractedFrom,
    /// `decision` supersedes a prior `decision`.        (Decision → Decision)
    Supersedes,
    /// `decision` records the rationale for any node.   (Decision → any)
    Decides,
    /// `api` exposes / implements an `operation`.       (Api → Operation)
    Implements,
    /// `entity` belongs to an `aggregate` root.         (Entity → Aggregate)
    BelongsTo,
    /// `variant` is a tagged case of an `entity`.       (Variant → Entity)
    VariantOf,
    /// `state_machine` transitions between states (intra-machine).
    Transitions,
    /// `access_pattern` queries / traverses an `entity`. (AccessPattern → Entity)
    Queries,
    /// `policy` authorizes an `operation` (or `api` route). (Policy → Operation|Api)
    Authorizes,
    /// `scope` contains a kernel node.                  (Scope → any)
    Contains,
    /// `intent` derives / refines another `intent`.     (Intent → Intent)
    DerivesFrom,
    /// `consumer_contract` consumes a behavior.         (ConsumerContract → behavior)
    Consumes,
    /// `selector` selects/filters a DTO or entity.      (Selector → DTO|Entity)
    Selects,
    /// `operation` or behavior emits a log event.       (behavior → LogEvent)
    EmitsLog,
}

impl EdgeKind {
    pub const fn canonical(self) -> &'static str {
        match self {
            EdgeKind::Realizes => "realizes",
            EdgeKind::Verifies => "verifies",
            EdgeKind::Triggers => "triggers",
            EdgeKind::Emits => "emits",
            EdgeKind::Handles => "handles",
            EdgeKind::Constrains => "constrains",
            EdgeKind::TracesTo => "traces_to",
            EdgeKind::ExtractedFrom => "extracted_from",
            EdgeKind::Supersedes => "supersedes",
            EdgeKind::Decides => "decides",
            EdgeKind::Implements => "implements",
            EdgeKind::BelongsTo => "belongs_to",
            EdgeKind::VariantOf => "variant_of",
            EdgeKind::Transitions => "transitions",
            EdgeKind::Queries => "queries",
            EdgeKind::Authorizes => "authorizes",
            EdgeKind::Contains => "contains",
            EdgeKind::DerivesFrom => "derives_from",
            EdgeKind::Consumes => "consumes",
            EdgeKind::Selects => "selects",
            EdgeKind::EmitsLog => "emits_log",
        }
    }

    pub const ALL: [EdgeKind; 21] = [
        EdgeKind::Realizes,
        EdgeKind::Verifies,
        EdgeKind::Triggers,
        EdgeKind::Emits,
        EdgeKind::Handles,
        EdgeKind::Constrains,
        EdgeKind::TracesTo,
        EdgeKind::ExtractedFrom,
        EdgeKind::Supersedes,
        EdgeKind::Decides,
        EdgeKind::Implements,
        EdgeKind::BelongsTo,
        EdgeKind::VariantOf,
        EdgeKind::Transitions,
        EdgeKind::Queries,
        EdgeKind::Authorizes,
        EdgeKind::Contains,
        EdgeKind::DerivesFrom,
        EdgeKind::Consumes,
        EdgeKind::Selects,
        EdgeKind::EmitsLog,
    ];

    pub const fn is_kernel(&self) -> bool {
        true
    }
}

impl fmt::Display for EdgeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.canonical())
    }
}

impl FromStr for EdgeKind {
    type Err = ParseKindError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "realizes" => EdgeKind::Realizes,
            "verifies" => EdgeKind::Verifies,
            "triggers" => EdgeKind::Triggers,
            "emits" => EdgeKind::Emits,
            "handles" => EdgeKind::Handles,
            "constrains" => EdgeKind::Constrains,
            "traces_to" => EdgeKind::TracesTo,
            "extracted_from" => EdgeKind::ExtractedFrom,
            "supersedes" => EdgeKind::Supersedes,
            "decides" => EdgeKind::Decides,
            "implements" => EdgeKind::Implements,
            "belongs_to" => EdgeKind::BelongsTo,
            "variant_of" => EdgeKind::VariantOf,
            "transitions" => EdgeKind::Transitions,
            "queries" => EdgeKind::Queries,
            "authorizes" => EdgeKind::Authorizes,
            "contains" => EdgeKind::Contains,
            "derives_from" => EdgeKind::DerivesFrom,
            "consumes" => EdgeKind::Consumes,
            "selects" => EdgeKind::Selects,
            "emits_log" => EdgeKind::EmitsLog,
            other => {
                return Err(ParseKindError {
                    input: other.to_string(),
                })
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_kind_round_trip_canonical() {
        for k in NodeKind::ALL {
            let s = k.canonical();
            assert_eq!(NodeKind::from_str(s).unwrap(), k);
            assert_eq!(format!("{k}"), s);
            assert!(k.is_kernel());
        }
    }

    #[test]
    fn edge_kind_round_trip_canonical() {
        for e in EdgeKind::ALL {
            let s = e.canonical();
            assert_eq!(EdgeKind::from_str(s).unwrap(), e);
            assert_eq!(format!("{e}"), s);
            assert!(e.is_kernel());
        }
    }

    #[test]
    fn node_kind_serde_snake_case() {
        let json = serde_json::to_string(&NodeKind::StateMachine).unwrap();
        assert_eq!(json, "\"state_machine\"");
        let back: NodeKind = serde_json::from_str(&json).unwrap();
        assert_eq!(back, NodeKind::StateMachine);
    }

    #[test]
    fn unknown_kind_is_rejected() {
        assert!(NodeKind::from_str("nope").is_err());
        assert!(EdgeKind::from_str("nope").is_err());
    }

    #[test]
    fn variant_counts_are_exactly_twenty_one() {
        assert_eq!(NodeKind::ALL.len(), 21);
        assert_eq!(EdgeKind::ALL.len(), 21);
    }
}
