# AGENTS.md

This file gives coding agents the current architecture and working rules for this repository.

## Project Overview

Passka is a local Auth Broker for AI agents.

The model is broker-first: local principals request access to provider resources, Passka evaluates policy, issues short-lived leases, and optionally proxies HTTP requests without exposing long-lived provider material to the agent.

## Core Commands

```bash
# Build the Rust workspace
cargo build

# Run all Rust tests
cargo test --workspace

# Build the macOS Swift app
cd app && swift build

# Run the local broker daemon
cargo run -p passka-cli -- broker serve --addr 127.0.0.1:8478

# Inspect default principals
cargo run -p passka-cli -- principal list

# Register a provider account
cargo run -p passka-cli -- account add openai-prod --provider openai --auth api_key --base-url https://api.openai.com

# Create a broker policy
cargo run -p passka-cli -- policy allow --principal principal:local-agent --account <account_id> --resource openai/models/* --actions read

# Request a short-lived access lease
cargo run -p passka-cli -- request --principal principal:local-agent --resource openai/models/gpt-4.1 --action read

# Proxy an HTTP request with a lease
cargo run -p passka-cli -- proxy --lease <lease_id> --method GET --path /v1/models

# Inspect audit history
cargo run -p passka-cli -- audit list --limit 20
```

## Architecture

### `passka-core`

Main broker library.

- `broker.rs`: Broker service layer. Owns account registration, policy creation, access decisions, leases, audit events, OAuth completion/refresh, secret reveal checks, and HTTP proxy execution.
- `types.rs`: Broker domain objects: `Principal`, `ProviderAccount`, `ResourceGrant`, `BrokerPolicy`, `AccessLease`, `AuditEvent`, `ProviderSecret`, `ApiKeyMaterial`, `OAuthMaterial`, and HTTP proxy request/response shapes.
- `store/keychain.rs`: Generic JSON/password helpers for macOS Keychain. New broker secrets use service `passka-broker`.
- `oauth.rs`: OAuth code exchange and refresh helpers operating on `OAuthMaterial`.

### `passka-cli`

CLI and local daemon.

- `cli.rs`: Top-level subcommands and argument definitions.
- `commands/broker.rs`: Local HTTP JSON daemon exposed by `passka broker serve`.
- `commands/account.rs`: Register/list/show/reveal/remove provider accounts.
- `commands/principal.rs`: Manage human/agent principals.
- `commands/policy.rs`: Create and list broker policies.
- `commands/access.rs`: Request leases and proxy HTTP requests.
- `commands/audit.rs`: Audit inspection.
- `commands/auth.rs` and `commands/refresh.rs`: OAuth flow helpers.

### `app/`

SwiftPM macOS app.

- `PasskaBridge`: A Swift wrapper around the broker CLI.
- `CredentialStore.swift`: App-side observable state for provider accounts, policies, and audit events.
- Views now present a broker console: provider accounts, sensitive field reveal after local authentication, and recent audit history.

Do not reintroduce direct app reads from macOS Keychain.

## Data Model Rules

- Long-lived provider material lives in macOS Keychain under service `passka-broker`.
- Broker state lives in `~/.config/passka/broker/state.json`.
- Agents should receive leases and proxied results, not long-lived API keys, refresh tokens, or OAuth access tokens.
- Human reveal is allowed only through explicit broker reveal APIs and app-side local authentication.
- Every grant, denial, proxy request, token refresh, and reveal should produce an audit event when implemented through broker code.

## HTTP Daemon

The local daemon is intended for agents, MCP bridges, and future VFS drivers.

```text
GET    /health
GET    /principals
POST   /principals
GET    /accounts
POST   /accounts
GET    /accounts/{account_id}
DELETE /accounts/{account_id}
POST   /accounts/{account_id}/reveal
GET    /policies
POST   /policies/allow
GET    /audit?limit=20
POST   /access/request
POST   /http/proxy
POST   /oauth/{account_id}/start
POST   /oauth/{account_id}/complete
POST   /oauth/{account_id}/refresh
```

Prefer adding new agent-facing capabilities to this daemon instead of creating ad hoc CLI-only credential access.

## Things To Avoid

Do not add alternate security boundaries:

- Secret-to-stdout workflows.
- Environment-variable injection as the primary agent integration path.
- Direct Swift reads from Keychain.
- Native app bindings that bypass the broker policy layer.
- Any parallel storage or authorization path outside `Broker`.

## Development Guidance

- Keep `Broker` as the single policy/security boundary.
- Prefer adding methods and tests in `passka-core/src/broker.rs` before wiring CLI or app UI.
- When adding provider support, model it as `ProviderAccount` plus provider-specific `ProviderSecret` material and proxy behavior.
- When adding agent integrations, use `AccessLease` and `/http/proxy` rather than returning secrets.
- When changing the app, keep it as a broker console and continue to require local authentication before reveal.
- Run `cargo test --workspace` after Rust changes and `cd app && swift build` after app changes.
