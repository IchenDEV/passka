# passka v2 Refactor Design

Date: 2026-03-04

## Goals

1. Simplify credential types from 6 to 4 focused types.
2. Add full OAuth authorization code flow with local callback server.
3. Remove all commands that risk leaking credentials to stdout.
4. Add `--redact` mode to exec for output sanitization.
5. Support multi-credential injection in a single exec call.

## Credential Types

### api_key

For API keys, including AK/SK dual-key patterns.

- Required: `key`
- Optional: `secret` (for AK/SK), `endpoint`
- Env vars: `{NAME}_API_KEY`, `{NAME}_API_SECRET` (when secret present)

### password

Username and password pairs.

- Required: `username`, `password`
- Optional: `url`
- Env vars: `{NAME}_USERNAME`, `{NAME}_PASSWORD`

### session

Browser headers, cookies, session data as key-value pairs.

- Required: dynamic KV pairs (header name -> header value)
- Metadata (non-sensitive, in index.json): `domain`, `expires`
- Env vars: `{NAME}_{HEADER_KEY}` per header (uppercased, dashes to underscores)
- Example: `{"Cookie": "abc", "X-CSRF-Token": "xyz"}` for name "jira" produces `JIRA_COOKIE`, `JIRA_X_CSRF_TOKEN`

### oauth

Standard OAuth 2.0 authorization code flow with token refresh.

- Required: `token` (empty on initial add, filled by `auth` command)
- Optional: `refresh_token`, `token_url`, `authorize_url`, `client_id`, `client_secret`, `redirect_uri`, `scopes`, `expires_at`
- Env vars: `{NAME}_TOKEN`

## OAuth Authorization Flow

New command: `passka auth <name>`

Prerequisites: credential already added via `passka add <name> --type oauth` with at least `authorize_url`, `token_url`, `client_id`, `client_secret`.

Flow:

1. Read stored OAuth config from Keychain.
2. Generate random `state` parameter for CSRF protection.
3. Start local HTTP server on `localhost:9876` (configurable via `--port`).
4. Open browser to `authorize_url?client_id=...&redirect_uri=http://localhost:9876/callback&response_type=code&state=...&scope=...`.
5. Wait for callback (max 5 minutes).
   - On success: validate state, POST to token_url to exchange code for token.
   - On timeout: fall back to manual mode — prompt user to paste the redirect URL.
6. Store `token`, `refresh_token`, `expires_at` in Keychain.
7. Shut down server, print "authorization successful" to stderr.

The `add` flow for oauth becomes two-step:
- Step 1 (`add`): store config (authorize_url, token_url, client_id, client_secret, redirect_uri, scopes)
- Step 2 (`auth`): perform authorization, obtain token

Dependencies: `axum` (HTTP server), `open` (browser launch), `rand` (state generation).

## Commands

### Removed

- `get` — credential values to stdout, leak risk
- `snippet` — generates code templates using `get`
- `env` — outputs export statements referencing `get`

### Retained

- `add` — interactive credential storage
- `auth` — (NEW) OAuth authorization flow
- `exec` — inject env vars and run command (enhanced)
- `list` — metadata listing
- `show` — masked value display
- `rm` — delete credential
- `update` — update field
- `refresh` — manual token refresh

### exec Enhancements

Multi-credential support:

```
passka exec openai github -- python script.py
```

All env vars from both credentials are merged into the child process.

`--redact` flag:

```
passka exec --redact openai -- curl https://api.openai.com/v1/models
```

When `--redact` is set:
- Capture child process stdout/stderr (instead of passthrough).
- Load all credential field values from Keychain.
- Replace any occurrence of a credential value in the output with `[REDACTED]`.
- Write sanitized output to own stdout/stderr.

Without `--redact`, behavior is unchanged (passthrough).

## AI Skill Updates

Privacy rules (hard constraints):
- NEVER use removed commands (get, snippet, env).
- ALWAYS use `passka exec` for credential injection.
- ALWAYS add `--redact` for operations where output returns to AI context.
- NEVER generate code that prints/logs sensitive environment variables.
- Use `passka show` for debugging auth issues (shows masked values only).

Updated type reference table with 4 types.
New scenarios: multi-credential exec, OAuth auth guidance, session header injection.

## Files to Modify

Core:
- `crates/passka-core/src/types.rs` — rewrite CredentialType enum
- `crates/passka-core/src/oauth.rs` — rename refresh_url to token_url, add auth flow logic

CLI:
- `crates/passka-cli/src/cli.rs` — remove Get/Snippet/Env, add Auth, modify Exec
- `crates/passka-cli/src/commands/mod.rs` — update dispatch
- `crates/passka-cli/src/commands/add.rs` — guided flows per type
- `crates/passka-cli/src/commands/exec.rs` — multi-cred + --redact
- `crates/passka-cli/src/commands/auth.rs` — (NEW) OAuth authorization
- DELETE: `commands/get.rs`, `commands/snippet.rs`

FFI + App:
- `crates/passka-ffi/src/lib.rs` — adapt to new types
- `app/Sources/PasskaApp/Views/*` — update for 4 types

Skill:
- `~/.cursor/skills/passka/SKILL.md` — full rewrite
- `~/.cursor/skills/passka/references/scenarios.md` — full rewrite

Docs:
- `README.md` — updated commands, types, examples
