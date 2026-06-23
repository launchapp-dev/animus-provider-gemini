//! `animus-provider-gemini` — the Animus provider for Google Gemini.
//!
//! This is a thin wrapper over the shared ACP client (`animus-provider-acp`).
//! It advertises `provider_tool = "gemini"` and pins the harness to
//! `gemini --acp`, so the kernel routes Gemini models here exactly as before
//! while the plugin drives the Gemini CLI over the Agent Client Protocol
//! (structured streaming + a native permission callback) instead of scraping
//! stdout. Every tool call is gated through `animus agent approve-hook` by the
//! ACP client.

use std::sync::Arc;

use animus_plugin_runtime::{run_provider, ProviderInfo, SessionBackendProvider};
use animus_provider_acp::backend::AcpSessionBackend;
use animus_provider_acp::config::AcpConfig;

const DEFAULT_MODEL: &str = "gemini-2.5-flash";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    emit_manifest_if_requested();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    // `GEMINI_DEFAULT_MODEL` overrides the fallback model; `GEMINI_BIN`
    // overrides the harness binary (default `gemini`). The harness is always
    // driven in ACP mode (`--acp`).
    let default_model = std::env::var("GEMINI_DEFAULT_MODEL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_MODEL.to_string());
    let bin = std::env::var("GEMINI_BIN")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "gemini".to_string());

    let config = AcpConfig::for_harness("gemini", bin, ["--acp"], default_model.clone());
    let backend = Arc::new(AcpSessionBackend::new(config));

    // `ProviderInfo` fields are `&'static str`; leak the (process-lifetime)
    // default model so a `GEMINI_DEFAULT_MODEL` override is honored.
    let default_model: &'static str = Box::leak(default_model.into_boxed_str());

    let info = ProviderInfo {
        plugin_name: env!("CARGO_PKG_NAME"),
        plugin_version: env!("CARGO_PKG_VERSION"),
        description: env!("CARGO_PKG_DESCRIPTION"),
        default_tool: "gemini",
        default_model,
    };

    run_provider(info, SessionBackendProvider::new(backend)).await
}

fn emit_manifest_if_requested() {
    if !std::env::args()
        .skip(1)
        .any(|arg| arg == "--manifest" || arg == "-m")
    {
        return;
    }

    let manifest = serde_json::json!({
        "name": env!("CARGO_PKG_NAME"),
        "version": env!("CARGO_PKG_VERSION"),
        "plugin_kind": "provider",
        "description": env!("CARGO_PKG_DESCRIPTION"),
        "protocol_version": animus_plugin_protocol::PROTOCOL_VERSION,
        "capabilities": [
            "agent/run",
            "agent/resume",
            "agent/cancel",
            "health/check"
        ],
        "env_required": [
            {
                "name": "GEMINI_BIN",
                "description": "Override the Gemini CLI binary (default `gemini`). Driven in ACP mode via `--acp`.",
                "required": false
            },
            {
                "name": "GEMINI_DEFAULT_MODEL",
                "description": "Fallback model used when an agent/run request omits a model.",
                "required": false
            },
            {
                "name": "GEMINI_API_KEY",
                "description": "API key for the Gemini harness when using API-key auth.",
                "sensitive": true,
                "required": false
            },
            {
                "name": "GOOGLE_API_KEY",
                "description": "Google API key alternative for the Gemini harness.",
                "sensitive": true,
                "required": false
            },
            {
                "name": "ANIMUS_BIN",
                "description": "Path to the `animus` binary used for the approve-hook approval gate (default: resolved on PATH).",
                "required": false
            }
        ]
    });
    println!(
        "{}",
        serde_json::to_string(&manifest).expect("serialize manifest")
    );
    std::process::exit(0);
}
