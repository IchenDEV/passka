---
name: passka-creds
description: "Manage credentials with passka. Use when the user asks to store a secret, add an API key, run a command with credentials, list saved credentials, or anything related to credential/secret management. Triggers on: 'add secret', 'store credential', 'save API key', 'run with credentials', 'list credentials', 'show passwords', '添加密钥', '存密码', '密码管理'."
---

# Passka Credential Manager

Passka stores credentials securely in the macOS Keychain. Two credential types: `secret` (generic key-value) and `oauth` (with token refresh).

## 1. Execute commands with secrets

Inject stored credentials as environment variables into a command:

```bash
passka exec <credential-name> -- <command>
```

Examples:
```bash
# Single credential
passka exec openai -- curl -H "Authorization: Bearer $OPENAI_API_KEY" https://api.openai.com/v1/models

# Multiple credentials
passka exec aws-prod github-token -- ./deploy.sh

# Disable output redaction (secrets may appear in output)
passka exec --no-redact openai -- echo $OPENAI_API_KEY
```

Env var naming: field keys are uppercased with the credential name as prefix. E.g., credential `openai` with field `api_key` → `$OPENAI_API_KEY`.

## 2. Add and manage secrets

### Add a new secret
```bash
passka add <name> --type secret [-d "description"]
```
This starts an interactive prompt for key-value pairs. All values are entered as hidden input.

### Add an OAuth credential
```bash
passka add <name> --type oauth [-d "description"]
passka auth <name>        # complete browser-based OAuth flow
passka refresh <name>     # manually refresh token
```

### List and inspect
```bash
passka list               # list all credentials
passka list --type secret # filter by type
passka show <name>        # show fields (values masked)
```

### Update and remove
```bash
passka update <name> --field <field-name>   # update a single field (interactive)
passka rm <name>                            # delete credential
```

## Workflow

When the user wants to **use a secret in a command**:
1. Run `passka list` to check if the credential exists
2. If not, guide them to add it with `passka add`
3. Run the command with `passka exec <name> -- <command>`

When the user wants to **add a secret**:
1. Ask for a credential name and description
2. Run `passka add <name> --type secret -d "<description>"`
3. The CLI will interactively prompt for key-value fields
4. Confirm with `passka show <name>`

When the user wants to **see their credentials**:
1. Run `passka list` and present the output as a formatted table
