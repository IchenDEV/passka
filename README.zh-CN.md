# passka

[English README](README.md)

Passka 是一个给 AI Agent 使用的本地身份护照。

它让 Agent 可以使用 OpenAI、GitHub、Slack、飞书，或者任意 HTTP API，但不需要把长期 API Key、OAuth refresh token 交给 Agent。真正的凭证留在你的机器上。Passka 会检查 Agent 是否有权限访问某个资源，发放一个短期访问租约，并可以代表 Agent 发起 HTTP 请求。

你可以把它理解成给 Agent 用的本地 1Password 式中转站：Agent 请求的是“能力”，不是“密钥”。

## 为什么需要它

AI Agent 经常需要调用真实服务。最简单但也最危险的做法，是把 API Key 放进环境变量里，然后祈祷它不会被打印、记录或泄露。Passka 想提供一个更安全的流程：

1. 你添加一个 provider account，比如 OpenAI 或 GitHub。
2. 你创建一条 policy，说明哪个本地 Agent 可以访问哪个资源。
3. Agent 向 Passka 请求访问这个资源。
4. 如果 policy 允许，Passka 发放一个短期 lease。
5. Passka 代理请求，所以长期凭证始终留在本地。
6. Passka 把发生过的授权、拒绝、代理请求和敏感字段查看记录到 audit log。

## Passka 保护什么

- 长期 provider 授权材料存放在 macOS Keychain，service 为 `passka-broker`。
- Broker 状态、policy、lease、audit log 存放在 `~/.config/passka/broker/state.json`。
- Agent 拿到的是 access lease 和代理请求结果，不是 API Key 或 refresh token。
- macOS App 里查看敏感字段需要本地生物识别或设备认证。

## 快速开始

启动本地 broker：

```bash
cargo run -p passka-cli -- broker serve --addr 127.0.0.1:8478
```

检查服务是否启动：

```bash
curl http://127.0.0.1:8478/health
```

添加一个 provider account：

```bash
cargo run -p passka-cli -- account add openai-prod \
  --provider openai \
  --auth api_key \
  --base-url https://api.openai.com
```

允许默认本地 Agent 读取 OpenAI 模型资源：

```bash
cargo run -p passka-cli -- policy allow \
  --principal principal:local-agent \
  --account <account_id> \
  --resource openai/models/* \
  --actions read \
  --lease-seconds 300
```

请求一个短期 lease：

```bash
cargo run -p passka-cli -- request \
  --principal principal:local-agent \
  --resource openai/models/gpt-4.1 \
  --action read \
  --environment local \
  --purpose "model discovery"
```

用这个 lease 代理一次请求：

```bash
cargo run -p passka-cli -- proxy \
  --lease <lease_id> \
  --method GET \
  --path /v1/models
```

查看 audit log：

```bash
cargo run -p passka-cli -- audit list --limit 20
```

## OAuth 账号

OAuth provider 需要先添加账号，再完成浏览器授权流程：

```bash
cargo run -p passka-cli -- account add slack-workspace \
  --provider slack \
  --auth oauth \
  --base-url https://slack.com/api

cargo run -p passka-cli -- auth <account_id>
```

Passka 会在本地保存和刷新 OAuth 授权材料。Agent 仍然通过 lease 和 proxy 访问服务，不会拿到 refresh token。

## OTP 账号

Passka 也可以保存 TOTP seed，并通过 broker reveal 路径生成当前一次性验证码：

```bash
cargo run -p passka-cli -- account add github-otp \
  --provider github \
  --auth otp

cargo run -p passka-cli -- account reveal <account_id> --field code --raw
```

OTP seed 仍然保存在 macOS Keychain。查看 `code` 或 `seed` 会进入 audit log，并遵守和其他敏感字段相同的人类 reveal 规则。

## macOS App

macOS App 是一个 broker 控制台：

- 按 provider 浏览账号。
- 添加 API Key、OAuth、OTP 和 opaque provider account。
- 通过本地认证后查看敏感字段。
- 查看账号最近的 audit history。

构建方式：

```bash
cd app && swift build
```

## 核心概念

| 术语 | 含义 |
| --- | --- |
| Principal | 谁在请求。通常是本地用户或本地 Agent。 |
| Provider account | Passka 可以使用的外部账号，比如 OpenAI 或 GitHub。 |
| Policy | 规则：谁可以使用哪个 provider account 访问哪些资源。 |
| Resource | 被访问的东西，比如 `openai/models/*`。 |
| Lease | 一次短期批准，用来执行某类动作。 |
| Proxy | Passka 代发 HTTP 请求，同时隐藏真实凭证。 |
| Audit event | 授权、拒绝、查看、刷新、代理请求等事件记录。 |

## HTTP API

Agent 和 MCP bridge 可以使用 `passka broker serve` 暴露的本地 JSON API：

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

通过 HTTP 请求 lease：

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

通过 HTTP 代理请求：

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

## 常用命令

```bash
cargo run -p passka-cli -- principal list
cargo run -p passka-cli -- principal add <name> --kind agent

cargo run -p passka-cli -- account list
cargo run -p passka-cli -- account show <account_id>
cargo run -p passka-cli -- account reveal <account_id> --field api_key
cargo run -p passka-cli -- account reveal <account_id> --field code --raw
cargo run -p passka-cli -- account remove <account_id>

cargo run -p passka-cli -- policy list
cargo run -p passka-cli -- policy allow --principal <principal_id> --account <account_id> --resource <pattern> --actions read

cargo run -p passka-cli -- request --principal <principal_id> --resource <resource> --action <action>
cargo run -p passka-cli -- proxy --lease <lease_id> --method GET --path /path

cargo run -p passka-cli -- audit list --limit 20
cargo run -p passka-cli -- broker serve
```

## 开发

```bash
cargo build
cargo test --workspace
cd app && swift build
```
