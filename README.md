# animus-provider-gemini

A [Google Gemini CLI](https://github.com/google-gemini/gemini-cli) provider plugin for [Animus](https://github.com/launchapp-dev/animus-cli).

> **Status:** Under construction — landing in Animus v0.4.x. This crate currently lives in the Animus core workspace at `crates/animus-provider-gemini/`; v0.4.x extracts it to this standalone repository.

## What this is

Animus v0.4.0 makes providers (LLM CLI wrappers) pluggable. This repository will ship `animus-provider-gemini`, a stdio plugin that wraps Google's Gemini CLI as an Animus provider. Any workflow phase that targets `tool: gemini` dispatches through this plugin.

## Install (planned)

```bash
animus plugin install animus-provider-gemini
```

The Animus daemon image bundles this plugin pre-installed.

## Workflow YAML usage

```yaml
agents:
  ui-specialist:
    model: gemini-3.1-pro-preview
    tool: gemini
    mcp_servers: ["animus"]
```

## Roadmap

- [ ] Extract from Animus core workspace at v0.4.x cut
- [ ] Publish `animus-provider-gemini` crate to crates.io
- [ ] Release binaries (macOS aarch64/x86_64, Linux x86_64) on tag
- [ ] Independent semver track
- [ ] CI exercises the contract test from `animus-protocol`

## Design

- **Protocol:** [`animus-plugin-protocol`](https://github.com/launchapp-dev/animus-protocol) (provider variant)
- **Naming:** repo, crate, and binary all named `animus-provider-gemini`
- **Core repo:** [Animus](https://github.com/launchapp-dev/animus-cli)

## License

MIT — see [LICENSE](LICENSE).
