use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// IDL Abstract Syntax Tree
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IdlDocument {
    pub version: String,
    pub module: Option<Module>,
    pub imports: Vec<String>,
    pub blocks: Vec<Block>,
    pub metadata: DocumentMetadata,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentMetadata {
    pub version: Option<String>,
    pub source: Option<SourceType>,
    pub lifecycle: Option<LifecycleType>,
    pub drift_policy: Option<DriftPolicy>,
    pub trace_policy: Option<TracePolicy>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Module {
    pub name: String,
    pub path: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceType {
    Spec,
    Code,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LifecycleType {
    Managed,
    Exploratory,
    Archived,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DriftPolicy {
    Fail,
    Warn,
    Ignore,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TracePolicy {
    Strict,
    Advisory,
    None,
}

/// Top-level block in an IDL document
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Block {
    Intent(IntentBlock),
    Scope(ScopeBlock),
    Entity(EntityBlock),
    Event(EventBlock),
    Rule(RuleBlock),
    Invariant(InvariantBlock),
    Decision(DecisionBlock),
    Verification(VerificationBlock),
    Mapping(MappingBlock),
    TraceLink(TraceLinkBlock),
    Dependency(DependencyBlock),
    Service(ServiceBlock),
    Infrastructure(InfrastructureBlock),
    Requires(RequiresBlock),
    UxFlow(UxFlowBlock),
    UxComponent(UxComponentBlock),
    Pattern(PatternBlock),
    Variant(VariantBlock),
    StateMachine(StateMachineBlock),
    Execution(ExecutionBlock),
    Localization(LocalizationBlock),
    Profile(ProfileBlock),
    Policy(PolicyBlock),
    Operation(OperationBlock),
    Job(JobBlock),
    Aggregate(AggregateBlock),
    Api(ApiBlock),
    Constraints(ConstraintsBlock),
    // Extension blocks for future-proofing
    Extension(ExtensionBlock),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntentBlock {
    pub name: String,
    pub goal: String,
    pub outcome: Option<String>,
    pub actors: Vec<String>,
    pub business_value: Option<String>,
    pub priority: Option<Priority>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScopeBlock {
    pub name: String,
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityBlock {
    pub name: String,
    pub description: Option<String>,
    pub properties: HashMap<String, TypeExpression>,
    pub invariants: Vec<String>,
    pub storage: Option<StorageBlock>,
    pub access_patterns: Vec<AccessPatternBlock>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StorageBlock {
    pub table: Option<String>,
    pub indexes: Vec<IndexDefinition>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexDefinition {
    pub name: String,
    pub fields: Vec<String>,
    pub unique: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccessPatternBlock {
    pub name: String,
    pub operation: String,
    pub key: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeExpression {
    pub base_type: String,
    pub optional: bool,
    pub collection: Option<CollectionType>,
    pub generic_args: Vec<TypeExpression>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CollectionType {
    List,
    Set,
    Map,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventBlock {
    pub name: String,
    pub payload: HashMap<String, TypeExpression>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuleBlock {
    pub name: String,
    pub when: String,
    pub then: String,
    pub category: Option<RuleCategory>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleCategory {
    Behavioral,
    Temporal,
    Conditional,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvariantBlock {
    pub name: String,
    pub expression: String,
    pub scope: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionBlock {
    pub name: String,
    pub question: String,
    pub answer: String,
    pub rationale: Option<String>,
    pub alternatives: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerificationBlock {
    pub name: String,
    pub scenarios: Vec<ScenarioBlock>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioBlock {
    pub name: String,
    pub given: Vec<String>,
    pub when: String,
    pub then: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MappingBlock {
    pub name: String,
    pub from: String,
    pub to: String,
    pub transform: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceLinkBlock {
    pub from: String,
    pub to: String,
    pub link_type: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DependencyBlock {
    pub name: String,
    pub dep_type: String,
    pub version: Option<String>,
    pub bindings: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServiceBlock {
    pub name: String,
    pub description: Option<String>,
    pub endpoints: Vec<EndpointBlock>,
    pub auth: Option<AuthBlock>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EndpointBlock {
    pub name: String,
    pub method: HttpMethod,
    pub path: String,
    pub request: Option<String>,
    pub response: Option<String>,
    pub errors: Vec<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
    Head,
    Options,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthBlock {
    pub auth_type: String,
    pub details: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InfrastructureBlock {
    pub name: String,
    pub infra_type: String,
    pub config: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequiresBlock {
    pub requirements: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UxFlowBlock {
    pub name: String,
    pub steps: Vec<String>,
    pub error_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UxComponentBlock {
    pub name: String,
    pub component_type: String,
    pub interactions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatternBlock {
    pub name: String,
    pub pattern_type: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariantBlock {
    pub name: String,
    pub variants: Vec<VariantCase>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariantCase {
    pub name: String,
    pub fields: HashMap<String, TypeExpression>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateMachineBlock {
    pub name: String,
    pub states: Vec<String>,
    pub initial: String,
    pub transitions: Vec<TransitionBlock>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransitionBlock {
    pub from: String,
    pub to: String,
    pub event: String,
    pub guard: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionBlock {
    pub name: String,
    pub platform: String,
    pub config: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalizationBlock {
    pub name: String,
    pub locale: String,
    pub translations: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileBlock {
    pub name: String,
    pub settings: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicyBlock {
    pub name: String,
    pub conditions: Vec<String>,
    pub actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OperationBlock {
    pub name: String,
    pub input: HashMap<String, TypeExpression>,
    pub output: Option<TypeExpression>,
    pub preconditions: Vec<String>,
    pub postconditions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobBlock {
    pub name: String,
    pub trigger: JobTrigger,
    pub operation: String,
    pub retry: Option<RetryConfig>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum JobTrigger {
    Schedule { cron: String },
    Event { event: String },
    Operation { operation: String },
    Manual,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub backoff: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AggregateBlock {
    pub name: String,
    pub entities: Vec<String>,
    pub root: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiBlock {
    pub name: String,
    pub description: Option<String>,
    pub base_path: String,
    pub endpoints: Vec<EndpointBlock>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConstraintsBlock {
    pub name: String,
    pub constraints: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtensionBlock {
    pub block_type: String,
    pub name: Option<String>,
    pub fields: HashMap<String, serde_json::Value>,
}
