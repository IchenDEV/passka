<div align="center">
  <h1>Passka</h1>
  <p><strong>Local credential vault and lease broker for AI agents.</strong></p>
  <p>
    <img alt="Rust" src="https://img.shields.io/badge/Rust-2021-f74c00?style=flat-square">
    <img alt="macOS" src="https://img.shields.io/badge/macOS-Keychain-111111?style=flat-square">
    <img alt="Local First" src="https://img.shields.io/badge/local--first-credential%20broker-2f855a?style=flat-square">
    <img alt="License" src="https://img.shields.io/badge/license-MIT-blue?style=flat-square">
  </p>
  <p>
    <a href="README.zh-CN.md">中文说明</a>
    · <a href="#quick-start">Quick Start</a>
    · <a href="#http-api">HTTP API</a>
    · <a href="#development">Development</a>
  </p>
</div>

Passka is a local password manager for machine credentials with a built-in broker layer for AI agents.

You store long-lived credentials once in macOS Keychain. Agents never receive those raw secrets. Instead, an agent asks for a short-lived lease, then uses that lease to access the upstream service through Passka's HTTP proxy or placeholder injection flow.

If an upstream key leaks, you rotate the credential inside Passka and keep the agent integration unchanged.

## Core Model

| Concept | Meaning |
| --- | --- |
| Credential Account | A stored credential such as an API key, OAuth app, OTP seed, or opaque secret bundle. |
| Agent | A local AI tool or automation that is allowed to use an account. |
| Lease | A short-lived access ticket issued for one account. |
| Access Method | Either Passka HTTP proxying or local placeholder injection during proxying. |
| Audit | A record of authorizations, denials, proxy requests, refreshes, and app-side reveals. |

## Why This Exists

Without a broker, AI tools often get raw API keys through environment variables or copied secrets. That is convenient, but fragile.

Passka keeps the safer parts local:

1. You add a credential through the macOS app or CLI.
2. The secret is stored in macOS Keychain under `passka-broker`.
3. You authorize a local agent to use that account.
4. The agent requests a short-lived lease for the account.
5. The agent uses the lease through Passka's proxy or placeholder replacement.
6. Passka records what happened in the audit log.

## Security Boundaries

- Long-lived secrets stay in macOS Keychain.
- Broker state lives in `~/.config/passka/broker/state.json`.
- CLI can add, list, show metadata, authorize, request leases, and proxy requests.
- CLI does not reveal secret material.
- The macOS app is the only human-facing secret reveal path, and it requires local authentication.
- Agents receive leases and proxied results, not raw API keys or refresh tokens.

## Install

GitHub Releases publish architecture-specific macOS artifacts for both the CLI and the desktop app:

- `passka-cli-<version>-macos-x86_64.tar.gz`
- `passka-cli-<version>-macos-arm64.tar.gz`
- `Passka-<version>-macos-x86_64.zip`
- `Passka-<version>-macos-arm64.zip`

Choose the archive that matches your Mac.

Install the CLI:

```bash
tar -xzf passka-cli-<version>-macos-<arch>.tar.gz
mkdir -p "$HOME/.local/bin"
mv passka "$HOME/.local/bin/passka"
chmod +x "$HOME/.local/bin/passka"
```

If `~/.local/bin` is not already on your `PATH`, add this line to `~/.zshrc`:

```bash
export PATH="$HOME/.local/bin:$PATH"
```

Install the macOS app:

1. Unzip `Passka-<version>-macos-<arch>.zip`
2. Drag `Passka.app` into `/Applications`
3. Right-click `Passka.app` and choose `Open` the first time if macOS warns that the app is unsigned

Release artifacts are currently unsigned and not notarized, so the first launch needs an explicit `Open` confirmation.

## Quick Start

Run the local broker:

```bash
cargo run -p passka-cli -- broker serve --addr 127.0.0.1:8478
```

Check health:

```bash
curl http://127.0.0.1:8478/health
```

Add an account:

```bash
cargo run -p passka-cli -- account add openai-prod \
  --provider openai \
  --auth api_key \
  --base-url https://api.openai.com
```

List existing principals:

```bash
cargo run -p passka-cli -- principal list
```

Passka bootstraps a default local agent as `principal:local-agent`. To register another agent:

```bash
cargo run -p passka-cli -- principal add my-agent \
  --kind agent \
  --description "My local AI agent"
```

Authorize the default local agent to use that account:

```bash
cargo run -p passka-cli -- account allow <account_id> \
  --agent principal:local-agent \
  --lease-seconds 300
```

Request a short-lived lease:

```bash
cargo run -p passka-cli -- request \
  --principal principal:local-agent \
  --account <account_id> \
  --environment local \
  --purpose "model discovery"
```

Use the lease through the direct proxy endpoint:

```bash
cargo run -p passka-cli -- proxy \
  --lease <lease_id> \
  --method GET \
  --path https://api.openai.com/v1/models
```

Inspect audit history:

```bash
cargo run -p passka-cli -- audit list --limit 20
```

## Supported Credential Types

Passka stores several credential shapes under the same account model:

- `api_key`: API key plus auth header metadata
- `oauth`: authorize locally, refresh locally, proxy without exposing refresh tokens
- `otp`: store a TOTP seed for human use in the macOS app
- `opaque`: arbitrary key-value secret bundles

These are storage types, not separate product concepts.

## Access Methods

Passka supports two ways for an agent to use a lease.

### 1. HTTP Proxy

The agent sends the upstream request to Passka, and Passka injects the real credential locally.

Direct proxy request:

```bash
curl -s http://127.0.0.1:8478/http/proxy \
  -H 'content-type: application/json' \
  -d '{
    "lease_id": "<lease_id>",
    "request": {
      "method": "GET",
      "path": "https://api.openai.com/v1/models"
    }
  }'
```

Forward proxy request:

```bash
curl -x http://127.0.0.1:8478 \
  --proxy-header "X-Passka-Lease: <lease_id>" \
  https://api.openai.com/v1/models
```

### 2. Placeholder Injection During Proxying

Passka can replace placeholders in forwarded headers or text bodies before sending the upstream request.

Primary placeholders:

- `PASSKA_API_KEY`
  Used for a primary API key credential.
- `PASSKA_TOKEN`
  Used for a primary OAuth access token.

Extra leased accounts can be bound to aliases:

```bash
cargo run -p passka-cli -- proxy \
  --lease <openai_lease_id> \
  --extra-lease github=<github_lease_id> \
  --extra-lease slack=<slack_lease_id> \
  --method POST \
  --path https://api.example.test/composite \
  --body '{"openai":"PASSKA_API_KEY","github":"PASSKA_GITHUB_API_KEY","slack":"PASSKA_SLACK_TOKEN","github_account":"PASSKA_GITHUB_ACCOUNT_ID"}'
```

Alias placeholders follow `PASSKA_${NAME}_${FIELD}`. Common examples:

- `PASSKA_GITHUB_API_KEY`
- `PASSKA_SLACK_TOKEN`
- `PASSKA_GITHUB_ACCOUNT_ID`

## macOS App

The macOS app is the human-facing credential manager:

- Browse stored credential accounts
- Add API key, OAuth, OTP, and opaque accounts
- Register agent principals
- Authorize stored accounts for agents
- Reveal sensitive values only after local authentication
- Inspect recent audit activity for an account

Build it with:

```bash
cd app && swift build
```

## HTTP API

The local daemon exposed by `passka broker serve` is intended for agents, MCP bridges, and the app.

```text
GET    /health
GET    /principals
POST   /principals
GET    /accounts
POST   /accounts
GET    /accounts/{account_id}
DELETE /accounts/{account_id}
POST   /accounts/{account_id}/authorize
GET    /authorizations
GET    /audit?limit=20
POST   /access/request
POST   /http/proxy
POST   /oauth/{account_id}/start
POST   /oauth/{account_id}/complete
POST   /oauth/{account_id}/refresh
POST   /app/accounts/{account_id}/reveal
```

Notes:

- `/accounts/{account_id}/authorize` binds an agent principal to one account.
- `/authorizations` lists the current account-to-agent authorization rules.
- `/access/request` now takes `principal_id` and `account_id`.
- `/app/accounts/{account_id}/reveal` is for the macOS app path, not for general agent use.

## Development

Build the Rust workspace:

```bash
cargo build
```

Run the Rust tests:

```bash
cargo test --workspace
```

Build the macOS app:

```bash
cd app && swift build
```

Create a GitHub release by pushing a semver tag such as `v0.1.0`. The release workflow builds macOS Intel and Apple Silicon artifacts, packages the CLI and desktop app, and uploads them to the GitHub Release with a `SHA256SUMS.txt` file.
