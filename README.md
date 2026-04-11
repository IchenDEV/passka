# passka

[中文说明](README.zh-CN.md)

Passka is a local passport for AI agents.

It lets an agent use services like OpenAI, GitHub, Slack, Feishu, or any HTTP API without handing the agent your long-lived API keys or OAuth refresh tokens. You keep the real credentials on your machine. Passka checks whether the agent is allowed to do the thing it asked for, creates a short-lived access lease, and can make the HTTP request on the agent's behalf.

Think of it like a local 1Password-style broker for agents: the agent asks for a capability, not a secret.

## Why Use It

AI agents often need to call real services. The unsafe version is simple: put API keys in environment variables and hope nothing prints or leaks them. Passka aims for a safer flow:

1. You add a provider account, such as OpenAI or GitHub.
2. You create a policy that says which local agent can access which resource.
3. The agent asks Passka for access to that resource.
4. Passka grants a short-lived lease if the policy allows it.
5. Passka proxies the request, so the long-lived credential stays local.
6. Passka records what happened in the audit log.

## What Passka Protects

- Long-lived provider material is stored in macOS Keychain under service `passka-broker`.
- Broker state, policies, leases, and audit logs live in `~/.config/passka/broker/state.json`.
- Agents receive access leases and proxied results, not API keys or refresh tokens.
- Human reveal in the macOS app requires local biometric/device authentication.

## Quick Start

Run the local broker:

```bash
cargo run -p passka-cli -- broker serve --addr 127.0.0.1:8478
```

Check that it is alive:

```bash
curl http://127.0.0.1:8478/health
```

Add a provider account:

```bash
cargo run -p passka-cli -- account add openai-prod \
  --provider openai \
  --auth api_key \
  --base-url https://api.openai.com
```

Allow the default local agent to read OpenAI model resources:

```bash
cargo run -p passka-cli -- policy allow \
  --principal principal:local-agent \
  --account <account_id> \
  --resource openai/models/* \
  --actions read \
  --lease-seconds 300
```

Ask for a short-lived lease:

```bash
cargo run -p passka-cli -- request \
  --principal principal:local-agent \
  --resource openai/models/gpt-4.1 \
  --action read \
  --environment local \
  --purpose "model discovery"
```

Use that lease to proxy a request:

```bash
cargo run -p passka-cli -- proxy \
  --lease <lease_id> \
  --method GET \
  --path /v1/models
```

See what happened:

```bash
cargo run -p passka-cli -- audit list --limit 20
```

## OAuth Accounts

For OAuth providers, add the account and then complete the browser-based authorization flow:

```bash
cargo run -p passka-cli -- account add slack-workspace \
  --provider slack \
  --auth oauth \
  --base-url https://slack.com/api

cargo run -p passka-cli -- auth <account_id>
```

Passka stores and refreshes OAuth material locally. Agents still request leases and proxy requests; they do not receive refresh tokens.

## macOS App

The macOS app is a broker console:

- Browse provider accounts by provider.
- Add API key, OAuth, and opaque provider accounts.
- Reveal sensitive fields only after local authentication.
- Inspect recent audit history for an account.

Build it with:

```bash
cd app && swift build
```

## Concepts

| Term | Plain meaning |
| --- | --- |
| Principal | Who is asking. Usually a local human or local agent. |
| Provider account | The external account Passka can use, such as an OpenAI or GitHub account. |
| Policy | The rule that says who can use which provider account for which resources. |
| Resource | The thing being accessed, such as `openai/models/*`. |
| Lease | A short-lived approval to do one kind of action. |
| Proxy | Passka making the HTTP request while keeping the secret hidden. |
| Audit event | A record of a grant, denial, reveal, refresh, or proxied request. |

## HTTP API

Agents and MCP bridges can use the local JSON API exposed by `passka broker serve`:

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

Request a lease:

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

Proxy a request:

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

## Useful Commands

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

## Development

```bash
cargo build
cargo test --workspace
cd app && swift build
```
