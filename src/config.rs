use anyhow::Result;

/// Runtime configuration for the Gemini provider plugin.
#[derive(Debug, Clone)]
pub struct GeminiConfig {
    /// Name (or absolute path) of the Gemini CLI binary the provider should
    /// spawn. Read from `GEMINI_BIN`; defaults to `"gemini"`.
    pub gemini_bin: String,
    /// Default model identifier when the `AgentRunRequest` doesn't specify
    /// one. Read from `GEMINI_DEFAULT_MODEL`; defaults to
    /// `"gemini-3.1-pro-preview"`.
    pub default_model: String,
}

impl GeminiConfig {
    /// Build a config from environment variables, applying defaults for any
    /// unset values.
    pub fn from_env() -> Result<Self> {
        let gemini_bin = std::env::var("GEMINI_BIN").unwrap_or_else(|_| "gemini".to_string());
        let default_model = std::env::var("GEMINI_DEFAULT_MODEL")
            .unwrap_or_else(|_| "gemini-3.1-pro-preview".to_string());

        Ok(Self {
            gemini_bin,
            default_model,
        })
    }

    /// Helper for integration tests / embedders that want to construct a
    /// config without going through env vars.
    pub fn for_testing(gemini_bin: impl Into<String>) -> Self {
        Self {
            gemini_bin: gemini_bin.into(),
            default_model: "gemini-3.1-pro-preview".to_string(),
        }
    }
}
