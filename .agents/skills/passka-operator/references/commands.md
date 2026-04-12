# Passka Commands

Use these examples when operating Passka from CLI or the local HTTP daemon.

## Broker lifecycle

Start the broker:

```bash
passka broker serve --addr 127.0.0.1:8478
```

Health check:

```bash
curl http://127.0.0.1:8478/health
```

## Principal management

List principals:

```bash
passka principal list
```

Add an agent principal:

```bash
passka principal add my-agent --kind agent --description "My local AI agent"
```

## Account management

Add an API key account:

```bash
passka account add openai-prod \
  --provider openai \
  --auth api_key \
  --base-url https://api.openai.com
```

List accounts:

```bash
passka account list
```

Show account metadata:

```bash
passka account show <account_id>
```

Authorize an account for an agent:

```bash
passka account allow <account_id> \
  --agent principal:local-agent \
  --lease-seconds 300
```

Remove an account:

```bash
passka account remove <account_id>
```

## Lease request and proxy

Request a lease:

```bash
passka request \
  --principal principal:local-agent \
  --account <account_id> \
  --environment local \
  --purpose "model discovery"
```

Proxy a direct request:

```bash
passka proxy \
  --lease <lease_id> \
  --method GET \
  --path https://api.openai.com/v1/models
```

Proxy a multi-account request:

```bash
passka proxy \
  --lease <openai_lease_id> \
  --extra-lease slack=<slack_lease_id> \
  --method POST \
  --path https://api.example.test/composite \
  --body '{"openai":"PASSKA_API_KEY","slack":"PASSKA_SLACK_TOKEN"}'
```

## Audit and OAuth

List audit events:

```bash
passka audit list --limit 20
```

Start OAuth setup:

```bash
passka auth <account_id>
```

Refresh an OAuth account:

```bash
passka refresh <account_id>
```

## HTTP API

Authorize an account for an agent:

```bash
curl -s http://127.0.0.1:8478/accounts/<account_id>/authorize \
  -H 'content-type: application/json' \
  -d '{
    "principal_id": "principal:local-agent",
    "environments": ["local"],
    "max_lease_seconds": 300,
    "description": "local agent access"
  }'
```

Request a lease:

```bash
curl -s http://127.0.0.1:8478/access/request \
  -H 'content-type: application/json' \
  -d '{
    "principal_id": "principal:local-agent",
    "account_id": "<account_id>",
    "context": {
      "environment": "local",
      "purpose": "model discovery",
      "source": "mcp"
    }
  }'
```

Send a direct proxy request:

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

## Placeholder summary

- Primary API key: `PASSKA_API_KEY`
- Primary OAuth token: `PASSKA_TOKEN`
- Aliased API key: `PASSKA_${NAME}_API_KEY`
- Aliased OAuth token: `PASSKA_${NAME}_TOKEN`
- Common metadata: `PASSKA_${NAME}_ACCOUNT_ID`, `PASSKA_${NAME}_ACCOUNT_NAME`, `PASSKA_${NAME}_PROVIDER`, `PASSKA_${NAME}_BASE_URL`

Alias example:

- `--extra-lease slack=<lease_id>` gives `PASSKA_SLACK_TOKEN`
- `--extra-lease github-app=<lease_id>` gives `PASSKA_GITHUB_APP_API_KEY`

## Deprecated or disallowed paths

- Do not use `passka policy allow`; it no longer exists
- Do not use `passka account reveal`
- Do not treat `/app/accounts/{account_id}/reveal` as a general agent endpoint
