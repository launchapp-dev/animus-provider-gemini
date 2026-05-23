use std::sync::Arc;
use std::time::Instant;

use animus_plugin_protocol::{HealthCheckResult, HealthStatus};
use animus_provider_protocol::{
    AgentNotification, AgentResumeRequest, AgentRunRequest, AgentRunResponse, BackendError,
    NotificationSink, ProviderBackend, ProviderCapabilities, ProviderManifest,
};
use animus_session_backend::{
    lookup_binary_in_path, GeminiSessionBackend, SessionBackend, SessionEvent, SessionRequest,
};
use async_trait::async_trait;

use crate::config::GeminiConfig;

/// Provider plugin backend that wraps a `SessionBackend` (the native Gemini
/// CLI driver by default) behind the `ProviderBackend` trait.
pub struct GeminiProviderBackend {
    session: Arc<dyn SessionBackend>,
    config: GeminiConfig,
}

impl GeminiProviderBackend {
    /// Build a backend with the bundled `GeminiSessionBackend` driver.
    pub fn new(config: GeminiConfig) -> Self {
        Self {
            session: Arc::new(GeminiSessionBackend::new()),
            config,
        }
    }

    /// Test/embedder constructor that lets callers inject any
    /// `SessionBackend` implementation (e.g. a fake for contract tests).
    pub fn with_session<S>(session: S, config: GeminiConfig) -> Self
    where
        S: SessionBackend + 'static,
    {
        Self {
            session: Arc::new(session),
            config,
        }
    }

    fn build_session_request(&self, request: &AgentRunRequest) -> SessionRequest {
        let model = request
            .model
            .clone()
            .unwrap_or_else(|| self.config.default_model.clone());

        let env_vars = request
            .env
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Vec<_>>();

        let mut extras = serde_json::Map::new();
        if let Some(contract) = &request.runtime_contract {
            extras.insert("runtime_contract".to_string(), contract.clone());
        }
        if let Some(mcp) = &request.mcp_servers {
            extras.insert("mcp_servers".to_string(), mcp.clone());
        }
        if let Some(tools) = &request.tools {
            extras.insert("tools".to_string(), tools.clone());
        }
        if let Some(schema) = &request.response_schema {
            extras.insert("response_schema".to_string(), schema.clone());
        }

        SessionRequest {
            tool: "gemini".to_string(),
            model,
            prompt: request.prompt.clone(),
            cwd: request.cwd.clone(),
            project_root: request.project_root.clone(),
            mcp_endpoint: None,
            permission_mode: request.permission_mode.clone(),
            timeout_secs: request.timeout_secs,
            env_vars,
            extras: serde_json::Value::Object(extras),
        }
    }

    async fn drain_events(
        &self,
        mut run: animus_session_backend::SessionRun,
        started: Instant,
        model_label: String,
        sink: NotificationSink,
    ) -> Result<AgentRunResponse, BackendError> {
        let mut output = String::new();
        let mut thinking: Vec<String> = Vec::new();
        let mut errors: Vec<String> = Vec::new();
        let mut tool_calls: Vec<serde_json::Value> = Vec::new();
        let mut tool_results: Vec<serde_json::Value> = Vec::new();
        let mut metadata: Vec<serde_json::Value> = Vec::new();
        let mut session_id = run.session_id.clone();
        let mut backend_label = run.selected_backend.clone();
        let mut exit_code: i32 = 0;

        while let Some(event) = run.events.recv().await {
            let current_session_id = session_id.clone().unwrap_or_default();
            match event {
                SessionEvent::Started {
                    backend,
                    session_id: id,
                    ..
                } => {
                    if !backend.is_empty() {
                        backend_label = backend;
                    }
                    if session_id.is_none() {
                        session_id = id;
                    }
                }
                SessionEvent::TextDelta { text } => {
                    output.push_str(&text);
                    sink.emit(AgentNotification::Output {
                        session_id: current_session_id,
                        text,
                        is_final: false,
                    });
                }
                SessionEvent::FinalText { text } => {
                    output = text.clone();
                    sink.emit(AgentNotification::Output {
                        session_id: current_session_id,
                        text,
                        is_final: true,
                    });
                }
                SessionEvent::Thinking { text } => {
                    thinking.push(text.clone());
                    sink.emit(AgentNotification::Thinking {
                        session_id: current_session_id,
                        text,
                    });
                }
                SessionEvent::ToolCall {
                    tool_name,
                    arguments,
                    server,
                } => {
                    tool_calls.push(serde_json::json!({
                        "tool_name": tool_name,
                        "arguments": arguments,
                        "server": server,
                    }));
                    sink.emit(AgentNotification::ToolCall {
                        session_id: current_session_id,
                        name: tool_name,
                        arguments,
                        server,
                    });
                }
                SessionEvent::ToolResult {
                    tool_name,
                    output: result,
                    success,
                } => {
                    tool_results.push(serde_json::json!({
                        "tool_name": tool_name,
                        "output": result,
                        "success": success,
                    }));
                    sink.emit(AgentNotification::ToolResult {
                        session_id: current_session_id,
                        name: tool_name,
                        output: result,
                        success,
                    });
                }
                SessionEvent::Artifact {
                    artifact_id,
                    metadata: artifact_metadata,
                } => {
                    metadata.push(serde_json::json!({
                        "artifact_id": artifact_id,
                        "metadata": artifact_metadata,
                    }));
                }
                SessionEvent::Metadata { metadata: meta } => {
                    metadata.push(meta);
                }
                SessionEvent::Error {
                    message,
                    recoverable,
                } => {
                    errors.push(message.clone());
                    if !recoverable {
                        exit_code = 1;
                    }
                    sink.emit(AgentNotification::Error {
                        session_id: current_session_id,
                        message,
                        recoverable,
                    });
                }
                SessionEvent::Finished { exit_code: code } => {
                    if let Some(code) = code {
                        exit_code = code;
                    }
                    break;
                }
            }
        }

        drop(sink);

        let session_id = session_id.unwrap_or_default();
        let backend_label = if backend_label.is_empty() {
            format!("gemini:{model_label}")
        } else {
            format!("gemini-native:{model_label}")
        };

        Ok(AgentRunResponse {
            session_id,
            exit_code,
            output,
            metadata,
            tool_calls,
            tool_results,
            thinking,
            errors,
            duration_ms: started.elapsed().as_millis() as u64,
            backend: backend_label,
            tokens_used: None,
            decision_verdict: None,
        })
    }
}

#[async_trait]
impl ProviderBackend for GeminiProviderBackend {
    fn manifest(&self) -> ProviderManifest {
        ProviderManifest {
            name: env!("CARGO_PKG_NAME").to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            description: env!("CARGO_PKG_DESCRIPTION").to_string(),
            supported_models: vec![
                "gemini-3.1-pro-preview".to_string(),
                "gemini-2.5-pro".to_string(),
                "gemini-2.5-flash".to_string(),
                "gemini-1.5-pro".to_string(),
                "gemini-1.5-flash".to_string(),
            ],
            tool: "gemini".to_string(),
            // The Gemini CLI itself is write-capable; whether a given workflow
            // phase routes here is a separate policy concern handled by the
            // Animus core (see `enforce_write_capable_phase_target`).
            capabilities: ProviderCapabilities {
                streaming: true,
                resume: true,
                cancellation: true,
                write_capable: true,
                mcp: true,
            },
        }
    }

    async fn run_agent(&self, request: AgentRunRequest) -> Result<AgentRunResponse, BackendError> {
        self.run_agent_streaming(request, NotificationSink::noop())
            .await
    }

    async fn run_agent_streaming(
        &self,
        request: AgentRunRequest,
        sink: NotificationSink,
    ) -> Result<AgentRunResponse, BackendError> {
        let started = Instant::now();
        let session_request = self.build_session_request(&request);
        let model_label = session_request.model.clone();
        let run = self
            .session
            .start_session(session_request)
            .await
            .map_err(|error| BackendError::SessionStartFailed(error.to_string()))?;
        self.drain_events(run, started, model_label, sink).await
    }

    async fn resume_agent(
        &self,
        request: AgentResumeRequest,
    ) -> Result<AgentRunResponse, BackendError> {
        self.resume_agent_streaming(request, NotificationSink::noop())
            .await
    }

    async fn resume_agent_streaming(
        &self,
        request: AgentResumeRequest,
        sink: NotificationSink,
    ) -> Result<AgentRunResponse, BackendError> {
        let started = Instant::now();
        let session_id = request
            .session_id
            .clone()
            .ok_or_else(|| BackendError::RunFailed("resume requires a session_id".to_string()))?;
        let session_request = self.build_session_request(&request);
        let model_label = session_request.model.clone();
        let run = self
            .session
            .resume_session(session_request, &session_id)
            .await
            .map_err(|error| BackendError::SessionStartFailed(error.to_string()))?;
        self.drain_events(run, started, model_label, sink).await
    }

    async fn cancel_agent(&self, session_id: &str) -> Result<(), BackendError> {
        self.session
            .terminate_session(session_id)
            .await
            .map_err(|error| BackendError::Other(anyhow::anyhow!(error.to_string())))
    }

    async fn health(&self) -> Result<HealthCheckResult, BackendError> {
        match lookup_binary_in_path(&self.config.gemini_bin) {
            Some(_) => Ok(HealthCheckResult {
                status: HealthStatus::Healthy,
                uptime_ms: None,
                memory_usage_bytes: None,
                last_error: None,
            }),
            None => Ok(HealthCheckResult {
                status: HealthStatus::Unhealthy,
                uptime_ms: None,
                memory_usage_bytes: None,
                last_error: Some(format!(
                    "gemini binary '{}' not found in PATH",
                    self.config.gemini_bin
                )),
            }),
        }
    }
}
