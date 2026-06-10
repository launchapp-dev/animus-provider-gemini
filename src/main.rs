use animus_plugin_protocol::{PluginInfo, PLUGIN_KIND_PROVIDER};
use animus_plugin_runtime::provider_main_with_capabilities;
use animus_provider_gemini::backend::GeminiProviderBackend;
use animus_provider_gemini::config::GeminiConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    emit_manifest_if_requested();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let config = GeminiConfig::from_env()?;
    let backend = GeminiProviderBackend::new(config);

    let info = PluginInfo {
        name: env!("CARGO_PKG_NAME").into(),
        version: env!("CARGO_PKG_VERSION").into(),
        plugin_kind: PLUGIN_KIND_PROVIDER.into(),
        description: Some(env!("CARGO_PKG_DESCRIPTION").into()),
    };

    let extra_capabilities = vec![];

    provider_main_with_capabilities(info, backend, extra_capabilities).await
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
                "description": "Override the Gemini CLI binary path.",
                "required": false
            },
            {
                "name": "GEMINI_DEFAULT_MODEL",
                "description": "Fallback model used when the request omits a model.",
                "required": false
            },
            {
                "name": "GEMINI_API_KEY",
                "description": "Gemini API key forwarded to the Gemini CLI.",
                "sensitive": true,
                "required": false
            },
            {
                "name": "GOOGLE_API_KEY",
                "description": "Google API key forwarded to the Gemini CLI.",
                "sensitive": true,
                "required": false
            },
            {
                "name": "GOOGLE_GENAI_USE_VERTEXAI",
                "description": "Enable Vertex AI mode for Google GenAI clients.",
                "required": false
            },
            {
                "name": "GOOGLE_CLOUD_PROJECT",
                "description": "Google Cloud project used by Vertex AI mode.",
                "required": false
            },
            {
                "name": "GOOGLE_CLOUD_LOCATION",
                "description": "Google Cloud region used by Vertex AI mode.",
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
