//! Thin wrapper around the OpenAI Responses API.
//!
//! The HTTP transport is hidden behind the [`HttpClient`] trait so unit tests
//! can assert against a fake without making real network calls. The default
//! transport ([`ReqwestClient`]) sends a `POST https://api.openai.com/v1/responses`
//! request authenticated by `OPENAI_API_KEY`.
//!
//! On success the model is expected to return a single JSON object matching
//! the `RoundResponse` shape (graph_delta + questions + decisions +
//! confidence_overall). Anything else is reported as an error so the round
//! retry loop can re-prompt with validation feedback.

use crate::{LlmProvider, RoundRequest, RoundResponse, ToolDef};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

const DEFAULT_ENDPOINT: &str = "https://api.openai.com/v1/responses";

#[async_trait]
pub trait HttpClient: Send + Sync {
    async fn post_json(&self, url: &str, api_key: &str, body: Value) -> Result<Value>;
}

pub struct ReqwestClient {
    inner: reqwest::Client,
}

impl ReqwestClient {
    pub fn new() -> Self {
        Self {
            inner: reqwest::Client::new(),
        }
    }
}

impl Default for ReqwestClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl HttpClient for ReqwestClient {
    async fn post_json(&self, url: &str, api_key: &str, body: Value) -> Result<Value> {
        let resp = self
            .inner
            .post(url)
            .bearer_auth(api_key)
            .json(&body)
            .send()
            .await
            .context("POST OpenAI Responses API")?;
        let status = resp.status();
        let text = resp.text().await.context("read response body")?;
        if !status.is_success() {
            return Err(anyhow!("OpenAI {}: {}", status, text));
        }
        serde_json::from_str(&text).context("parse OpenAI response JSON")
    }
}

pub struct OpenAiProvider {
    api_key: String,
    endpoint: String,
    http: Arc<dyn HttpClient>,
}

impl OpenAiProvider {
    /// Construct from `OPENAI_API_KEY`. Returns an error if the env var is unset.
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .context("OPENAI_API_KEY env var is required for OpenAiProvider")?;
        Ok(Self::new(api_key, Arc::new(ReqwestClient::new())))
    }

    pub fn new(api_key: String, http: Arc<dyn HttpClient>) -> Self {
        Self {
            api_key,
            endpoint: DEFAULT_ENDPOINT.into(),
            http,
        }
    }

    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }

    fn build_body(&self, req: &RoundRequest) -> Value {
        json!({
            "model": req.model,
            "instructions": req.system,
            "input": req.user,
            "tools": req.tools.iter().map(tool_to_openai_schema).collect::<Vec<_>>(),
            "response_format": { "type": "json_object" },
            "metadata": {
                "session_id": req.session_id,
                "round": req.round
            }
        })
    }
}

fn tool_to_openai_schema(t: &ToolDef) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": t.name,
            "description": t.description,
            "parameters": t.parameters
        }
    })
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    async fn run_round(&self, request: RoundRequest) -> Result<RoundResponse> {
        let body = self.build_body(&request);
        let raw = self
            .http
            .post_json(&self.endpoint, &self.api_key, body)
            .await?;
        extract_round_response(&raw)
    }
}

/// Pull a `RoundResponse` out of the Responses API envelope. The model is
/// instructed to return one JSON object; we accept either the new
/// `output_text` shortcut or the legacy `output[0].content[0].text` chain.
fn extract_round_response(raw: &Value) -> Result<RoundResponse> {
    let text = if let Some(s) = raw.get("output_text").and_then(Value::as_str) {
        s.to_string()
    } else if let Some(arr) = raw.get("output").and_then(Value::as_array) {
        arr.iter()
            .find_map(|item| {
                item.get("content")?
                    .as_array()?
                    .iter()
                    .find_map(|c| c.get("text").and_then(Value::as_str).map(str::to_string))
            })
            .ok_or_else(|| anyhow!("no text payload in OpenAI response"))?
    } else {
        return Err(anyhow!("OpenAI response missing output_text/output"));
    };

    let parsed: RoundResponse =
        serde_json::from_str(&text).context("parse model JSON as RoundResponse")?;
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::default_tools;
    use std::sync::Mutex;

    struct FakeHttp {
        captured: Mutex<Option<(String, String, Value)>>,
        reply: Value,
    }

    #[async_trait]
    impl HttpClient for FakeHttp {
        async fn post_json(&self, url: &str, api_key: &str, body: Value) -> Result<Value> {
            *self.captured.lock().unwrap() = Some((url.into(), api_key.into(), body));
            Ok(self.reply.clone())
        }
    }

    #[tokio::test]
    async fn forwards_request_and_decodes_output_text() {
        let reply = json!({
            "output_text": "{\"graph_delta\":{\"version\":\"0.1.0\",\"nodes\":[],\"edges\":[]},\"questions\":[],\"decisions\":[],\"confidence_overall\":0.42}"
        });
        let http = Arc::new(FakeHttp {
            captured: Mutex::new(None),
            reply,
        });
        let provider = OpenAiProvider::new("sk-test".into(), http.clone());
        let resp = provider
            .run_round(RoundRequest {
                system: "sys".into(),
                user: "usr".into(),
                tools: default_tools(),
                round: 1,
                session_id: "s1".into(),
                model: "gpt-5.5".into(),
                validation_feedback: None,
            })
            .await
            .unwrap();
        assert!((resp.confidence_overall - 0.42).abs() < 1e-9);
        let (_, key, body) = http.captured.lock().unwrap().clone().unwrap();
        assert_eq!(key, "sk-test");
        assert_eq!(body["model"], "gpt-5.5");
        assert_eq!(body["tools"].as_array().unwrap().len(), 5);
    }
}
