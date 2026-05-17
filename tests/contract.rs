use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use animus_plugin_protocol::HealthStatus;
use animus_provider_gemini::backend::GeminiProviderBackend;
use animus_provider_gemini::config::GeminiConfig;
use animus_provider_protocol::{AgentRunRequest, ProviderBackend};
use animus_session_backend::{
    Result as SessionResult, SessionBackend, SessionBackendInfo, SessionBackendKind,
    SessionCapabilities, SessionEvent, SessionRequest, SessionRun, SessionStability,
};
use async_trait::async_trait;
use tokio::sync::mpsc;

// ---------------------------------------------------------------------
// Fakes
// ---------------------------------------------------------------------

struct FakeSession {
    started: Arc<Mutex<Vec<SessionRequest>>>,
    resumed: Arc<Mutex<Vec<(SessionRequest, String)>>>,
    cancelled: Arc<Mutex<Vec<String>>>,
    canned: Vec<SessionEvent>,
}

impl FakeSession {
    fn new(canned: Vec<SessionEvent>) -> Self {
        Self {
            started: Arc::new(Mutex::new(Vec::new())),
            resumed: Arc::new(Mutex::new(Vec::new())),
            cancelled: Arc::new(Mutex::new(Vec::new())),
            canned,
        }
    }

    fn started_log(&self) -> Arc<Mutex<Vec<SessionRequest>>> {
        self.started.clone()
    }

    fn resumed_log(&self) -> Arc<Mutex<Vec<(SessionRequest, String)>>> {
        self.resumed.clone()
    }

    fn cancelled_log(&self) -> Arc<Mutex<Vec<String>>> {
        self.cancelled.clone()
    }

    async fn emit_run(&self) -> SessionRun {
        let (tx, rx) = mpsc::channel(32);
        for event in self.canned.clone() {
            let _ = tx.send(event).await;
        }
        let _ = tx.send(SessionEvent::Finished { exit_code: Some(0) }).await;
        drop(tx);
        SessionRun {
            session_id: Some("fake-session-id".to_string()),
            events: rx,
            selected_backend: "gemini-fake".to_string(),
            fallback_reason: None,
            pid: None,
        }
    }
}

#[async_trait]
impl SessionBackend for FakeSession {
    fn info(&self) -> SessionBackendInfo {
        SessionBackendInfo {
            kind: SessionBackendKind::GeminiSdk,
            provider_tool: "gemini".to_string(),
            stability: SessionStability::Experimental,
            display_name: "Fake Gemini Backend".to_string(),
        }
    }

    fn capabilities(&self) -> SessionCapabilities {
        SessionCapabilities {
            supports_resume: true,
            supports_terminate: true,
            supports_permissions: true,
            supports_mcp: true,
            supports_tool_events: false,
            supports_thinking_events: false,
            supports_artifact_events: false,
            supports_usage_metadata: true,
        }
    }

    async fn start_session(&self, request: SessionRequest) -> SessionResult<SessionRun> {
        self.started.lock().unwrap().push(request);
        Ok(self.emit_run().await)
    }

    async fn resume_session(
        &self,
        request: SessionRequest,
        session_id: &str,
    ) -> SessionResult<SessionRun> {
        self.resumed
            .lock()
            .unwrap()
            .push((request, session_id.to_string()));
        Ok(self.emit_run().await)
    }

    async fn terminate_session(&self, session_id: &str) -> SessionResult<()> {
        self.cancelled.lock().unwrap().push(session_id.to_string());
        Ok(())
    }
}

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

fn run_request(model: Option<&str>, prompt: &str) -> AgentRunRequest {
    AgentRunRequest {
        session_id: None,
        prompt: prompt.to_string(),
        model: model.map(|s| s.to_string()),
        system_prompt: None,
        cwd: PathBuf::from("/tmp"),
        project_root: None,
        permission_mode: None,
        timeout_secs: None,
        env: HashMap::new(),
        mcp_servers: None,
        tools: None,
        response_schema: None,
        runtime_contract: None,
        extras: HashMap::new(),
    }
}

fn resume_request(session_id: &str, prompt: &str) -> AgentRunRequest {
    let mut request = run_request(Some("gemini-3.1-pro-preview"), prompt);
    request.session_id = Some(session_id.to_string());
    request
}

// ---------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------

#[tokio::test]
async fn run_agent_via_fake_session() {
    let fake = FakeSession::new(vec![SessionEvent::FinalText {
        text: "hello".to_string(),
    }]);
    let started_log = fake.started_log();
    let backend =
        GeminiProviderBackend::with_session(fake, GeminiConfig::for_testing("/usr/bin/true"));

    let response = backend
        .run_agent(run_request(Some("gemini-3.1-pro-preview"), "ping"))
        .await
        .expect("run_agent should succeed");

    assert!(
        response.output.contains("hello"),
        "expected output to contain 'hello', got {:?}",
        response.output
    );
    assert_eq!(response.exit_code, 0);
    assert_eq!(response.session_id, "fake-session-id");
    assert!(response.backend.contains("gemini"));

    let started = started_log.lock().unwrap();
    assert_eq!(started.len(), 1);
    assert_eq!(started[0].tool, "gemini");
    assert_eq!(started[0].model, "gemini-3.1-pro-preview");
    assert_eq!(started[0].prompt, "ping");
}

#[tokio::test]
async fn resume_agent_via_fake_session() {
    let fake = FakeSession::new(vec![SessionEvent::FinalText {
        text: "resumed".to_string(),
    }]);
    let resumed_log = fake.resumed_log();
    let backend =
        GeminiProviderBackend::with_session(fake, GeminiConfig::for_testing("/usr/bin/true"));

    let response = backend
        .resume_agent(resume_request("prior-session-xyz", "keep going"))
        .await
        .expect("resume_agent should succeed");

    assert!(response.output.contains("resumed"));

    let resumed = resumed_log.lock().unwrap();
    assert_eq!(resumed.len(), 1);
    assert_eq!(resumed[0].1, "prior-session-xyz");
}

#[tokio::test]
async fn cancel_agent_forwards_session_id() {
    let fake = FakeSession::new(Vec::new());
    let cancelled_log = fake.cancelled_log();
    let backend =
        GeminiProviderBackend::with_session(fake, GeminiConfig::for_testing("/usr/bin/true"));

    backend
        .cancel_agent("session-to-cancel")
        .await
        .expect("cancel_agent should succeed");

    let cancelled = cancelled_log.lock().unwrap();
    assert_eq!(cancelled.as_slice(), &["session-to-cancel".to_string()]);
}

#[tokio::test]
async fn health_unhealthy_when_gemini_missing() {
    let backend = GeminiProviderBackend::new(GeminiConfig::for_testing(
        "/definitely/does/not/exist/animus-gemini-bin",
    ));
    let health = backend.health().await.expect("health should not error");
    assert_eq!(health.status, HealthStatus::Unhealthy);
    assert!(health.last_error.is_some());
    let last_error = health.last_error.unwrap();
    assert!(
        last_error.contains("not found"),
        "expected error to mention 'not found', got {last_error:?}"
    );
}

#[tokio::test]
async fn health_healthy_when_gemini_present() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    let tempdir = tempfile::tempdir().expect("create tempdir");
    let bin_path = tempdir.path().join("gemini");
    fs::write(&bin_path, "#!/bin/sh\nexit 0\n").expect("write stub binary");
    let mut perms = fs::metadata(&bin_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&bin_path, perms).unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", tempdir.path().display(), original_path);

    // SAFETY: tests in this binary that touch PATH run sequentially because
    // `cargo test` defaults to one thread for env-mutating cases is not
    // guaranteed — but for this contract suite the env mutation only
    // affects `which` lookups for `"gemini"`, and we restore it before
    // returning.
    std::env::set_var("PATH", &new_path);

    let backend = GeminiProviderBackend::new(GeminiConfig::for_testing("gemini"));
    let health = backend.health().await.expect("health should not error");

    std::env::set_var("PATH", original_path);

    assert_eq!(health.status, HealthStatus::Healthy);
    assert!(health.last_error.is_none());
}

#[tokio::test]
async fn manifest_capabilities_sanity() {
    let backend = GeminiProviderBackend::new(GeminiConfig::for_testing("gemini"));
    let manifest = backend.manifest();

    assert_eq!(manifest.name, "animus-provider-gemini");
    assert_eq!(manifest.tool, "gemini");
    assert!(!manifest.version.is_empty());
    assert!(!manifest.description.is_empty());
    assert!(manifest
        .supported_models
        .iter()
        .any(|m| m == "gemini-3.1-pro-preview"));

    let caps = manifest.capabilities;
    assert!(caps.streaming);
    assert!(caps.resume);
    assert!(caps.cancellation);
    assert!(caps.write_capable);
    assert!(caps.mcp);
}
