# animus-provider-gemini

A [Google Gemini CLI](https://github.com/google-gemini/gemini-cli) provider plugin for [Animus](https://github.com/launchapp-dev/animus-cli).

## What this is

A stdio provider plugin that exposes Google's Gemini CLI as an Animus provider. Any workflow phase that targets `tool: gemini` dispatches through this plugin.

As of v0.3.0 it drives the Gemini CLI over the **Agent Client Protocol (ACP)** — `gemini --acp` — rather than scraping stdout. It is a thin wrapper over the shared ACP client ([`animus-provider-acp`](https://github.com/launchapp-dev/animus-provider-acp)), pinned to the Gemini harness and advertising `provider_tool = "gemini"`. This gives structured streaming + tool events and a **native permission callback**, with every tool call gated through `animus agent approve-hook`. ACP is an internal transport detail; the kernel still routes Gemini models to `tool: gemini` exactly as before.

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
