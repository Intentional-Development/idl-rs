//! IDL MCP Server — binary entry point.
//!
//! Exposes the IDL semantic graph to LLM agents via the Model Context Protocol.
//! Transport: stdio (standard for local MCP servers).

use anyhow::Result;
use rmcp::{transport::stdio, ServiceExt};
use tracing_subscriber::{self, EnvFilter};

use idl_mcp_server::IdlServer;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    tracing::info!("Starting IDL MCP server");

    let service = match IdlServer::new().serve(stdio()).await {
        Ok(service) => service,
        Err(error) => {
            tracing::error!("serving error: {:?}", error);
            return Err(error.into());
        }
    };

    service.waiting().await?;
    Ok(())
}
