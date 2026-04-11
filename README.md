# passka

Passka is a local Auth Broker for AI agents.

Passka stores provider authorization material locally, evaluates policy for a principal, issues short-lived access leases, and can proxy HTTP requests on the agent's behalf. Agents do not receive long-lived secrets.

## Architecture

Passka vNext is built around broker-first objects:

| Object | Meaning |
| --- | --- |
| `Principal` | A local human or agent identity, such as `principal:local-human` or `principal:local-agent`. |
| `ProviderAccount` | A bound SaaS/API account, such as OpenAI, GitHub, Slack, Feishu, or a generic API provider. |
| `ResourceGrant` | A resource pattern and allowed actions for one provider account. |
| `BrokerPolicy` | Connects a principal to one or more grants, with lease duration and reveal permissions. |
| `AccessLease` | A short-lived capability issued after policy approval. |
| `AuditEvent` | A record of account registration, policy changes, access decisions, proxy calls, reveals, and OAuth actions. |

Storage is split by sensitivity:

| Data | Storage |
| --- | --- |
| Long-lived provider material | macOS Keychain, service `passka-broker` |
| Broker state, policies, leases, audit | `~/.config/passka/broker/state.json` |

## Quick Start

Run the local daemon:

```bash
cargo run -p passka-cli -- broker serve --addr 127.0.0.1:8478
```

Check health:

```bash
curl http://127.0.0.1:8478/health
```

List built-in principals:

```bash
cargo run -p passka-cli -- principal list
```

Register an API-key provider account:

```bash
cargo run -p passka-cli -- account add openai-prod \
  --provider openai \
  --auth api_key \
  --base-url https://api.openai.com
```

Register an OAuth provider account:

```bash
cargo run -p passka-cli -- account add slack-workspace \
  --provider slack \
  --auth oauth \
  --base-url https://slack.com/api

cargo run -p passka-cli -- auth <account_id>
```

Allow an agent to access a resource:

```bash
cargo run -p passka-cli -- policy allow \
  --principal principal:local-agent \
  --account <account_id> \
  --resource openai/models/* \
  --actions read \
  --lease-seconds 300
```

Request a lease:

```bash
cargo run -p passka-cli -- request \
  --principal principal:local-agent \
  --resource openai/models/gpt-4.1 \
  --action read \
  --environment local \
  --purpose "model discovery"
```

Proxy an HTTP request through an approved lease:

```bash
cargo run -p passka-cli -- proxy \
  --lease <lease_id> \
  --method GET \
  --path /v1/models
```

Inspect audit history:

```bash
cargo run -p passka-cli -- audit list --limit 20
```

## Broker HTTP API

The daemon exposes a local JSON API for agents, MCP bridges, and future VFS drivers:

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

Request a lease over HTTP:

```bash
curl -s http://127.0.0.1:8478/access/request \
  -H 'content-type: application/json' \
  -d '{
    "principal_id": "principal:local-agent",
    "resource": "openai/models/gpt-4.1",
    "action": "read",
    "context": {
      "environment": "local",
      "purpose": "model discovery",
      "source": "mcp"
    }
  }'
```

Proxy a request over HTTP:

```bash
curl -s http://127.0.0.1:8478/http/proxy \
  -H 'content-type: application/json' \
  -d '{
    "lease_id": "<lease_id>",
    "request": {
      "method": "GET",
      "path": "/v1/models"
    }
  }'
```

## CLI Map

```bash
cargo run -p passka-cli -- principal list
cargo run -p passka-cli -- principal add <name> --kind agent

cargo run -p passka-cli -- account list
cargo run -p passka-cli -- account show <account_id>
cargo run -p passka-cli -- account reveal <account_id> --field api_key
cargo run -p passka-cli -- account remove <account_id>

cargo run -p passka-cli -- policy list
cargo run -p passka-cli -- policy allow --principal <principal_id> --account <account_id> --resource <pattern> --actions read

cargo run -p passka-cli -- request --principal <principal_id> --resource <resource> --action <action>
cargo run -p passka-cli -- proxy --lease <lease_id> --method GET --path /path

cargo run -p passka-cli -- audit list --limit 20
cargo run -p passka-cli -- broker serve
```

## macOS App

The app is now a broker console:

- Browse provider accounts by provider.
- Add API key, OAuth, and opaque provider accounts.
- Reveal sensitive fields only after local biometric/device authentication.
- Inspect recent audit history for an account.
- Use the broker CLI bridge for all broker operations.

Build it with:

```bash
cd app && swift build
```

## Development

```bash
cargo build
cargo test --workspace
cd app && swift build
```
