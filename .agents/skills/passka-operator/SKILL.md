---
name: passka-operator
description: Operate Passka through its CLI and local broker HTTP API. Use when Codex needs to manage Passka credential accounts, register or inspect agent principals, authorize an account for an agent, request a lease, proxy an upstream HTTP request, inspect audit history, run OAuth setup or refresh, or explain Passka placeholder injection such as PASSKA_API_KEY, PASSKA_TOKEN, PASSKA_${NAME}_API_KEY, and PASSKA_${NAME}_TOKEN.
---

# Passka Operator

## Overview

Use this skill to operate a local Passka installation safely. Prefer account-based authorization and lease flows, keep long-lived secrets inside Passka, and avoid deprecated or human-only secret access paths.

## Workflow

1. Confirm the execution surface before acting.
   Check whether `passka` is available on `PATH`. When working inside the Passka repo, also consider `target/debug/passka`.
   If the task uses the HTTP daemon, check `http://127.0.0.1:8478/health` first and start the broker if needed.

2. Choose the right interface.
   Use CLI for human/operator tasks: account management, principal management, account authorization, audit inspection, and OAuth setup.
   Use HTTP API for agent-style workflows and integration testing: lease requests, proxy requests, and local broker automation.

3. Keep the security boundary intact.
   Do not use `policy allow`; the old policy model has been removed.
   Do not use `account reveal`; human secret reveal belongs to the macOS app path.
   Prefer `account allow` plus `request --account` plus proxying over any secret-export flow.

4. Use account-based authorization.
   Register or identify the target agent principal.
   Authorize the credential account for that agent.
   Request a short-lived lease for the specific `account_id`.
   Use the lease in CLI proxy mode or broker HTTP mode.

## CLI Tasks

Use CLI for these tasks:

- List or add principals
- Add, list, show, or remove accounts
- Authorize an account for an agent with `account allow`
- Request a lease with `request --principal ... --account ...`
- Proxy a request with `proxy --lease ...`
- Inspect audit history
- Start OAuth setup with `auth <account_id>`
- Refresh OAuth credentials with `refresh <account_id>`

Read [references/commands.md](references/commands.md) for exact command patterns.

## HTTP Tasks

Use the broker HTTP API for these tasks:

- Broker health checks
- Account listing and registration
- Account authorization through `/accounts/{account_id}/authorize`
- Lease requests through `/access/request`
- Direct proxy requests through `/http/proxy`
- OAuth start, complete, and refresh

When using HTTP:

- Send `principal_id` plus `account_id` for lease requests.
- Treat `/app/accounts/{account_id}/reveal` as app-only; do not use it for general agent work.
- For forward-proxy mode, provide `X-Passka-Lease` or `Proxy-Authorization: Bearer <lease_id>`.

## Placeholder Rules

Use these placeholder rules when constructing proxy requests:

- `PASSKA_API_KEY` is for a primary API key credential.
- `PASSKA_TOKEN` is for a primary OAuth access token.
- `PASSKA_${NAME}_API_KEY` is for an aliased API key lease.
- `PASSKA_${NAME}_TOKEN` is for an aliased OAuth lease.
- Metadata placeholders such as `PASSKA_${NAME}_ACCOUNT_ID`, `PASSKA_${NAME}_ACCOUNT_NAME`, `PASSKA_${NAME}_PROVIDER`, and `PASSKA_${NAME}_BASE_URL` are also available.

Create aliased placeholders by binding extra leases with an alias such as `--extra-lease slack=<lease_id>` or JSON `extra_leases: { "slack": "<lease_id>" }`. Passka normalizes aliases to uppercase with non-alphanumeric characters converted to `_`.

## Guidance

- Prefer explicit account names like `openai-prod` or `slack-workspace-a` when helping users create accounts; it makes multi-account lease flows much easier to reason about.
- If the user is troubleshooting, inspect `principal list`, `account list`, account authorization, and the latest audit events before changing anything.
- If the user asks how Passka works, explain it as `Credential Account -> Lease -> Proxy/Injection -> Audit`.
- If the user asks for a command or request body, prefer concise copy-paste-ready examples over abstract descriptions.
