//! IDL MCP Server handler — tools, resources, and protocol implementation.

use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars,
    service::RequestContext,
    tool, tool_handler, tool_router,
};
use serde_json::json;

use idl_graph::doc::GraphDoc;
use idl_graph::extensions_dto::{parse_dtos, DtoDefinition};
use idl_proposals::{
    Proposal, ProposalStatus, DiffOp, audit_log, list_proposals,
    find_proposal, accept_proposal_safe, generate_proposal_id, locate_changes_dir,
};

// ---------- Tool input schemas (all pub for tests) ----------

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GraphReadParams {
    /// Path to the IDL graph JSON file
    pub path: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ListDtosParams {
    /// Path to the IDL graph JSON file
    pub path: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DtoGetParams {
    /// Path to the IDL graph JSON file
    pub path: String,
    /// Name (id) of the DTO to retrieve
    pub name: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ListEndpointsParams {
    /// Path to the IDL graph JSON file
    pub path: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SchemaVersionParams {
    /// Path to the IDL graph JSON file
    pub path: String,
}

// ---------- Proposal tool schemas ----------

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ProposalCreateParams {
    /// Path to the target IDL graph JSON file
    pub target_graph: String,
    /// Author identity (e.g., agent name, user email)
    pub author: String,
    /// Optional rationale for the change
    pub rationale: Option<String>,
    /// Diff operations (JSON array of diff op objects)
    pub diff_ops: Vec<serde_json::Value>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ProposalListParams {
    /// Optional status filter: "pending", "accepted", or "rejected"
    pub status: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ProposalGetParams {
    /// Proposal ID (full or prefix)
    pub id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ProposalAcceptParams {
    /// Proposal ID (full or prefix)
    pub id: String,
    /// Actor identity (e.g., agent name, user email)
    pub actor: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ProposalRejectParams {
    /// Proposal ID (full or prefix)
    pub id: String,
    /// Rejection reason
    pub reason: String,
    /// Actor identity (e.g., agent name, user email)
    pub actor: String,
}

// ---------- Server ----------

#[derive(Clone)]
pub struct IdlServer {
    tool_router: ToolRouter<IdlServer>,
}

#[tool_router]
impl IdlServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    /// Read and return the full IDL graph as JSON.
    #[tool(name = "idl.graph.read", description = "Parse an IDL graph file and return the full graph as JSON")]
    fn graph_read(
        &self,
        Parameters(params): Parameters<GraphReadParams>,
    ) -> Result<CallToolResult, McpError> {
        let doc = load_graph(&params.path)?;
        let json = serde_json::to_string_pretty(&doc)
            .map_err(|e| McpError::internal_error(format!("serialize: {e}"), None))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// List all DTO names and their kinds from the graph.
    #[tool(name = "idl.graph.list_dtos", description = "List DTO names and kinds from an IDL graph file")]
    fn list_dtos(
        &self,
        Parameters(params): Parameters<ListDtosParams>,
    ) -> Result<CallToolResult, McpError> {
        let doc = load_graph(&params.path)?;
        let dtos = extract_dtos(&doc);
        let summary: Vec<serde_json::Value> = dtos
            .iter()
            .map(|d| {
                json!({
                    "name": d.id,
                    "kind": format!("{:?}", d.kind),
                })
            })
            .collect();
        let json = serde_json::to_string_pretty(&summary)
            .map_err(|e| McpError::internal_error(format!("serialize: {e}"), None))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Get a single DTO by name.
    #[tool(name = "idl.dto.get", description = "Return a single DTO definition by name from an IDL graph file")]
    fn dto_get(
        &self,
        Parameters(params): Parameters<DtoGetParams>,
    ) -> Result<CallToolResult, McpError> {
        let doc = load_graph(&params.path)?;
        let dtos = extract_dtos(&doc);
        let dto = dtos.iter().find(|d| d.id == params.name).ok_or_else(|| {
            McpError::invalid_params(format!("DTO '{}' not found", params.name), None)
        })?;
        let json = serde_json::to_string_pretty(dto)
            .map_err(|e| McpError::internal_error(format!("serialize: {e}"), None))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// List endpoints (operation nodes) from the graph.
    #[tool(name = "idl.graph.list_endpoints", description = "List endpoint/operation nodes from an IDL graph file")]
    fn list_endpoints(
        &self,
        Parameters(params): Parameters<ListEndpointsParams>,
    ) -> Result<CallToolResult, McpError> {
        let doc = load_graph(&params.path)?;
        let endpoints: Vec<serde_json::Value> = doc
            .nodes
            .iter()
            .filter(|n| n.kind == "operation" || n.kind == "endpoint")
            .map(|n| {
                json!({
                    "id": n.id,
                    "kind": n.kind,
                    "state": n.state,
                    "props": n.props,
                })
            })
            .collect();
        let json = serde_json::to_string_pretty(&endpoints)
            .map_err(|e| McpError::internal_error(format!("serialize: {e}"), None))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Return the schema version from the graph metadata.
    #[tool(name = "idl.schema.version", description = "Return the schema version from an IDL graph file")]
    fn schema_version(
        &self,
        Parameters(params): Parameters<SchemaVersionParams>,
    ) -> Result<CallToolResult, McpError> {
        let doc = load_graph(&params.path)?;
        Ok(CallToolResult::success(vec![Content::text(doc.version)]))
    }

    /// Create a new proposal (MCP mutation tool).
    #[tool(name = "idl.proposal.create", description = "Create a new proposal for graph changes")]
    fn proposal_create(
        &self,
        Parameters(params): Parameters<ProposalCreateParams>,
    ) -> Result<CallToolResult, McpError> {
        // Parse diff_ops from JSON values to DiffOp enum
        let diff_ops: Vec<DiffOp> = params
            .diff_ops
            .into_iter()
            .map(|v| serde_json::from_value(v).map_err(|e| {
                McpError::invalid_params(format!("invalid diff_op: {}", e), None)
            }))
            .collect::<Result<Vec<_>, _>>()?;

        // Generate proposal ID
        let slug = params
            .rationale
            .as_ref()
            .and_then(|r| r.split_whitespace().next())
            .unwrap_or("change");
        let id = generate_proposal_id(slug);

        // Create proposal
        let proposal = Proposal::new(
            id.clone(),
            params.author.clone(),
            params.target_graph.clone(),
            params.rationale,
            diff_ops,
        );

        // Save to disk
        let changes_dir = locate_changes_dir()
            .map_err(|e| McpError::internal_error(format!("locate changes dir: {}", e), None))?;
        let proposal_path = changes_dir.join(format!("{}.proposal.json", id));
        proposal
            .save(&proposal_path)
            .map_err(|e| McpError::internal_error(format!("save proposal: {}", e), None))?;

        // Audit log
        audit_log("create", &id, &params.author, Some("mcp"), None)
            .map_err(|e| McpError::internal_error(format!("audit log: {}", e), None))?;

        let result = json!({
            "id": id,
            "path": proposal_path.display().to_string(),
            "status": "pending"
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result)
                .map_err(|e| McpError::internal_error(format!("serialize: {}", e), None))?,
        )]))
    }

    /// List proposals (optionally filtered by status).
    #[tool(name = "idl.proposal.list", description = "List proposals, optionally filtered by status")]
    fn proposal_list(
        &self,
        Parameters(params): Parameters<ProposalListParams>,
    ) -> Result<CallToolResult, McpError> {
        let status_filter = params.status.as_deref().map(|s| match s {
            "pending" => Ok(ProposalStatus::Pending),
            "accepted" => Ok(ProposalStatus::Accepted),
            "rejected" => Ok(ProposalStatus::Rejected),
            _ => Err(McpError::invalid_params(
                format!("invalid status filter: {}", s),
                None,
            )),
        }).transpose()?;

        let proposals = list_proposals(status_filter)
            .map_err(|e| McpError::internal_error(format!("list proposals: {}", e), None))?;

        let result: Vec<serde_json::Value> = proposals
            .into_iter()
            .map(|(path, p)| {
                json!({
                    "id": p.id,
                    "status": format!("{:?}", p.status).to_lowercase(),
                    "author": p.author,
                    "target_graph": p.target_graph,
                    "created_at": p.created_at.to_rfc3339(),
                    "ops_count": p.diff_ops.len(),
                    "path": path.display().to_string(),
                })
            })
            .collect();

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result)
                .map_err(|e| McpError::internal_error(format!("serialize: {}", e), None))?,
        )]))
    }

    /// Get a single proposal by ID.
    #[tool(name = "idl.proposal.get", description = "Get a proposal by ID (full or prefix)")]
    fn proposal_get(
        &self,
        Parameters(params): Parameters<ProposalGetParams>,
    ) -> Result<CallToolResult, McpError> {
        let (path, proposal) = find_proposal(&params.id)
            .map_err(|e| McpError::invalid_params(format!("{}", e), None))?;

        let result = json!({
            "id": proposal.id,
            "version": proposal.version,
            "status": format!("{:?}", proposal.status).to_lowercase(),
            "author": proposal.author,
            "target_graph": proposal.target_graph,
            "rationale": proposal.rationale,
            "diff_ops": proposal.diff_ops,
            "created_at": proposal.created_at.to_rfc3339(),
            "updated_at": proposal.updated_at.map(|t| t.to_rfc3339()),
            "rejection_reason": proposal.rejection_reason,
            "path": path.display().to_string(),
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result)
                .map_err(|e| McpError::internal_error(format!("serialize: {}", e), None))?,
        )]))
    }

    /// Accept a proposal (applies changes to target graph).
    #[tool(name = "idl.proposal.accept", description = "Accept a proposal and apply changes to the target graph")]
    fn proposal_accept(
        &self,
        Parameters(params): Parameters<ProposalAcceptParams>,
    ) -> Result<CallToolResult, McpError> {
        let hash = accept_proposal_safe(&params.id, &params.actor, Some("mcp"))
            .map_err(|e| McpError::internal_error(format!("accept proposal: {}", e), None))?;

        let result = json!({
            "id": params.id,
            "status": "accepted",
            "actor": params.actor,
            "graph_hash": hash,
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result)
                .map_err(|e| McpError::internal_error(format!("serialize: {}", e), None))?,
        )]))
    }

    /// Reject a proposal (does not modify the target graph).
    #[tool(name = "idl.proposal.reject", description = "Reject a proposal with a reason")]
    fn proposal_reject(
        &self,
        Parameters(params): Parameters<ProposalRejectParams>,
    ) -> Result<CallToolResult, McpError> {
        let (proposal_path, mut proposal) = find_proposal(&params.id)
            .map_err(|e| McpError::invalid_params(format!("{}", e), None))?;

        if proposal.status != ProposalStatus::Pending {
            return Err(McpError::invalid_params(
                format!(
                    "proposal {} is not pending (status: {:?})",
                    proposal.id, proposal.status
                ),
                None,
            ));
        }

        proposal.reject(params.reason.clone());
        proposal
            .save(&proposal_path)
            .map_err(|e| McpError::internal_error(format!("save proposal: {}", e), None))?;

        audit_log(
            "reject",
            &proposal.id,
            &params.actor,
            Some("mcp"),
            Some(json!({"reason": params.reason})),
        )
        .map_err(|e| McpError::internal_error(format!("audit log: {}", e), None))?;

        let result = json!({
            "id": proposal.id,
            "status": "rejected",
            "actor": params.actor,
            "reason": params.reason,
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result)
                .map_err(|e| McpError::internal_error(format!("serialize: {}", e), None))?,
        )]))
    }
}

#[tool_handler]
impl ServerHandler for IdlServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_resources()
                .enable_tools()
                .build(),
        )
        .with_server_info(Implementation::new("idl-mcp-server", env!("CARGO_PKG_VERSION")))
        .with_protocol_version(ProtocolVersion::V_2024_11_05)
        .with_instructions(
            "IDL MCP Server. Provides read tools to query IDL semantic graphs and mutation tools \
             to manage proposals. Read tools: idl.graph.read, idl.graph.list_dtos, idl.dto.get, \
             idl.graph.list_endpoints, idl.schema.version. Proposal tools: idl.proposal.create, \
             idl.proposal.list, idl.proposal.get, idl.proposal.accept, idl.proposal.reject. \
             Resources: idl://{path} for graph files, idl://schema for the JSON Schema."
                .to_string(),
        )
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult {
            resources: vec![RawResource::new(
                "idl://schema",
                "IDL Semantic Graph JSON Schema".to_string(),
            )
            .no_annotation()],
            next_cursor: None,
            meta: None,
        })
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        Ok(ListResourceTemplatesResult {
            resource_templates: vec![RawResourceTemplate {
                uri_template: "idl://{path}".to_string(),
                name: "IDL Graph File".to_string(),
                title: None,
                description: Some("Read an IDL semantic graph file by path".to_string()),
                mime_type: Some("application/json".to_string()),
                icons: None,
            }
            .no_annotation()],
            next_cursor: None,
            meta: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        let uri = request.uri.as_str();

        if uri == "idl://schema" {
            // Try to locate the schema file relative to common locations
            let schema_paths = [
                "schemas/semantic-graph.schema.json",
                "../IDL/schemas/semantic-graph.schema.json",
                "../../IDL/schemas/semantic-graph.schema.json",
            ];
            for path in &schema_paths {
                if let Ok(content) = std::fs::read_to_string(path) {
                    return Ok(ReadResourceResult::new(vec![ResourceContents::text(
                        content,
                        request.uri,
                    )]));
                }
            }
            return Err(McpError::resource_not_found(
                "Schema file not found. Set working directory to repo root.",
                None,
            ));
        }

        // Handle idl://{path} URIs
        if let Some(path) = uri.strip_prefix("idl://") {
            let doc = load_graph(path)?;
            let json = serde_json::to_string_pretty(&doc)
                .map_err(|e| McpError::internal_error(format!("serialize: {e}"), None))?;
            return Ok(ReadResourceResult::new(vec![ResourceContents::text(
                json,
                request.uri,
            )]));
        }

        Err(McpError::resource_not_found(
            format!("Unknown resource URI: {uri}"),
            None,
        ))
    }
}

// ---------- Helpers ----------

fn load_graph(path: &str) -> Result<GraphDoc, McpError> {
    GraphDoc::load(path).map_err(|e| {
        McpError::invalid_params(format!("Failed to load graph '{}': {}", path, e), None)
    })
}

fn extract_dtos(doc: &GraphDoc) -> Vec<DtoDefinition> {
    parse_dtos(doc).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_graph_json() -> String {
        serde_json::to_string_pretty(&json!({
            "version": "0.1.0",
            "metadata": {},
            "nodes": [
                {
                    "id": "op-get-users",
                    "kind": "operation",
                    "state": "accepted",
                    "props": { "method": "GET", "path": "/users" },
                    "source_anchors": [],
                    "decision_refs": []
                },
                {
                    "id": "entity-user",
                    "kind": "entity",
                    "state": "accepted",
                    "props": { "name": "User" },
                    "source_anchors": [],
                    "decision_refs": []
                }
            ],
            "edges": [
                {
                    "id": "edge-1",
                    "kind": "returns",
                    "from": "op-get-users",
                    "to": "entity-user",
                    "props": {}
                }
            ],
            "extensions": {
                "dto": {
                    "definitions": [
                        {
                            "id": "UserResponse",
                            "base": "entity-user",
                            "kind": "object",
                            "state": "accepted",
                            "created_by": "extractor",
                            "pick": ["id", "name", "email"]
                        }
                    ]
                }
            }
        }))
        .unwrap()
    }

    fn write_sample_graph() -> String {
        let dir = std::env::temp_dir();
        let path = dir.join("idl-mcp-test-graph.json");
        std::fs::write(&path, sample_graph_json()).unwrap();
        path.to_string_lossy().to_string()
    }

    fn extract_text(result: &CallToolResult) -> String {
        let content = &result.content[0];
        match &content.raw {
            rmcp::model::RawContent::Text(t) => t.text.clone(),
            _ => panic!("expected text content"),
        }
    }

    #[test]
    fn test_server_info() {
        let server = IdlServer::new();
        let info = server.get_info();
        assert_eq!(info.server_info.name, "idl-mcp-server");
    }

    #[test]
    fn test_list_tools() {
        let router = IdlServer::tool_router();
        let tools = router.list_all();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"idl.graph.read"));
        assert!(names.contains(&"idl.graph.list_dtos"));
        assert!(names.contains(&"idl.dto.get"));
        assert!(names.contains(&"idl.graph.list_endpoints"));
        assert!(names.contains(&"idl.schema.version"));
        // Proposal tools
        assert!(names.contains(&"idl.proposal.create"));
        assert!(names.contains(&"idl.proposal.list"));
        assert!(names.contains(&"idl.proposal.get"));
        assert!(names.contains(&"idl.proposal.accept"));
        assert!(names.contains(&"idl.proposal.reject"));
        assert_eq!(names.len(), 10);
    }

    #[test]
    fn test_graph_read() {
        let path = write_sample_graph();
        let server = IdlServer::new();
        let result = server.graph_read(Parameters(GraphReadParams { path })).unwrap();
        assert!(!result.is_error.unwrap_or(false));
    }

    #[test]
    fn test_list_endpoints() {
        let path = write_sample_graph();
        let server = IdlServer::new();
        let result = server.list_endpoints(Parameters(ListEndpointsParams { path })).unwrap();
        let text = extract_text(&result);
        assert!(text.contains("op-get-users"));
    }

    #[test]
    fn test_schema_version() {
        let path = write_sample_graph();
        let server = IdlServer::new();
        let result = server.schema_version(Parameters(SchemaVersionParams { path })).unwrap();
        let text = extract_text(&result);
        assert_eq!(text, "0.1.0");
    }

    #[test]
    fn test_list_dtos() {
        let path = write_sample_graph();
        let server = IdlServer::new();
        let result = server.list_dtos(Parameters(ListDtosParams { path })).unwrap();
        let text = extract_text(&result);
        assert!(text.contains("UserResponse"));
    }

    #[test]
    fn test_dto_get() {
        let path = write_sample_graph();
        let server = IdlServer::new();
        let result = server.dto_get(Parameters(DtoGetParams {
            path,
            name: "UserResponse".to_string(),
        })).unwrap();
        let text = extract_text(&result);
        assert!(text.contains("UserResponse"));
    }

    #[test]
    fn test_dto_get_not_found() {
        let path = write_sample_graph();
        let server = IdlServer::new();
        let result = server.dto_get(Parameters(DtoGetParams {
            path,
            name: "NonExistent".to_string(),
        }));
        assert!(result.is_err());
    }
}
