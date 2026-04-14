# AGENTS.md

This file gives coding agents the current architecture and working rules for this repository.

## Project Overview

Passka is a local credential vault and lease broker for AI agents.

The model is account-first: register an account, allow an agent principal to use it, issue an agent token, request a short-lived lease, and proxy upstream HTTP through the broker without exposing long-lived provider material to the agent.

## Core Commands

```bash
# Build the Rust workspace
cargo build

# Run all Rust tests
cargo test --workspace

# Build the macOS Swift app
cd app && swift build

# Run the local broker daemon
cargo run -p passka-cli -- broker serve

# Inspect default principals
cargo run -p passka-cli -- principal list

# Register a provider account
cargo run -p passka-cli -- account add openai-prod --provider openai --auth api_key --base-url https://api.openai.com

# Authorize an account for an agent
cargo run -p passka-cli -- account allow <account_id> --agent principal:local-agent --lease-seconds 300

# Optionally scope the authorization more tightly
cargo run -p passka-cli -- account allow <account_id> --agent principal:local-agent --allow-host api.openai.com --allow-method GET,POST --allow-path-prefix /v1/models,/v1/chat --lease-seconds 300

# Issue an agent token for a principal
cargo run -p passka-cli -- principal token issue principal:local-agent

# Request a short-lived access lease
cargo run -p passka-cli -- request --account <account_id> --agent-token <agent_token>

# Proxy an HTTP request with a lease
cargo run -p passka-cli -- proxy --lease <lease_id> --agent-token <agent_token> --method GET --path /v1/models

# Override daemon discovery when needed
PASSKA_BROKER_URL=http://127.0.0.1:9000 cargo run -p passka-cli -- request --account <account_id> --agent-token <agent_token>

# Inspect audit history
cargo run -p passka-cli -- audit list --limit 20
```

## Architecture

### `passka-core`

Main broker library.

- `broker.rs`: Broker service layer. Owns account registration, agent token issuance/authentication, account authorization, scoped access decisions, leases, audit events, OAuth completion/refresh, and HTTP proxy execution.
- `types.rs`: Broker domain objects: `Principal`, `ProviderAccount`, `AccountAuthorization`, `AccessLease`, `AuditEvent`, `ProviderSecret`, `ApiKeyMaterial`, `OAuthMaterial`, and HTTP proxy request/response shapes.
- `store/keychain.rs`: Generic JSON/password helpers for macOS Keychain. New broker secrets use service `passka-broker`.
- `oauth.rs`: OAuth code exchange and refresh helpers operating on `OAuthMaterial`.

### `passka-cli`

CLI and local daemon.

- `cli.rs`: Top-level subcommands and argument definitions.
- `commands/broker.rs`: Local HTTP daemon for the default agent plane exposed by `passka broker serve`.
- `commands/account.rs`: Register/list/show/remove provider accounts and authorize them for agents.
- `commands/principal.rs`: Manage human/agent principals and issue/revoke agent tokens.
- `commands/access.rs`: Agent-facing daemon client for requesting leases and proxying HTTP requests. It auto-discovers the last started daemon and also accepts explicit broker URL overrides.
- `commands/audit.rs`: Audit inspection.
- `commands/auth.rs` and `commands/refresh.rs`: OAuth flow helpers.

### `app/`

SwiftPM macOS app.

- `PasskaBridge`: A Swift wrapper around the broker CLI.
- `CredentialStore.swift`: App-side observable state for provider accounts, principals, and audit events.
- Views focus on account browsing/creation, agent registration, and audit visibility.

Do not reintroduce direct app reads from macOS Keychain.

## Data Model Rules

- Long-lived provider material lives in macOS Keychain under service `passka-broker`.
- Broker state lives in `~/.config/passka/broker/state.json`.
- The last daemon URL is written to `runtime.json` in the same broker config directory so agent-facing CLI commands can follow port changes.
- Agent tokens are hashed in broker state and are only returned once at issuance time.
- Leases snapshot scope from the selected authorization. Host and path scope default to `account.base_url` when explicit scope is omitted.
- Agents should receive leases and proxied results, not long-lived API keys, refresh tokens, or OAuth access tokens.
- Proxy requests may replace `PASSKA_API_KEY` or `PASSKA_TOKEN` in forwarded headers and UTF-8 text bodies for the primary leased account only.
- There is currently no secret reveal surface in the CLI, app, or daemon.
- Every authorization, denial, proxy request, and token refresh should produce an audit event when implemented through broker code.

## HTTP Daemon

The local daemon is the default agent plane for agents, MCP bridges, and future VFS drivers.

```text
GET    /health
POST   /access/request
POST   /http/proxy
```

Prefer adding new agent-facing capabilities to this daemon instead of creating ad hoc CLI-only credential access.
Authenticate JSON API calls with `Authorization: Bearer <agent_token>`.
Authenticate forward proxy requests with `X-Passka-Agent-Token: <agent_token>`.
When `127.0.0.1:8478` is busy, `passka broker serve` should fall back to a free local port and update `runtime.json` so `request` and `proxy` keep working.

## Things To Avoid

Do not add alternate security boundaries:

- Secret-to-stdout workflows.
- Environment-variable injection as the primary agent integration path.
- Accepting caller-supplied `principal_id` on the daemon.
- Direct Swift reads from Keychain.
- Native app bindings that bypass the broker authorization layer.
- Any parallel storage or authorization path outside `Broker`.

## Development Guidance

- Keep `Broker` as the single authorization/security boundary.
- Prefer adding methods and tests in `passka-core/src/broker.rs` before wiring CLI or app UI.
- When adding provider support, model it as `ProviderAccount` plus provider-specific `ProviderSecret` material and proxy behavior.
- When adding agent integrations, use `AccessLease` and `/http/proxy` rather than returning secrets, and preserve scoped lease enforcement on every proxied request.
- Keep the default daemon agent-only unless there is an explicit need for a separate hardened admin transport.
- When changing the app, keep it as a broker console and do not reintroduce secret reveal or a second admin transport through the current app bridge.
- Run `cargo test --workspace` after Rust changes and `cd app && swift build` after app changes.
