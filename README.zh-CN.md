<div align="center">
  <h1>Passka</h1>
  <p><strong>给 AI Agent 用的本地凭证库与 Lease Broker。</strong></p>
  <p>
    <img alt="Rust" src="https://img.shields.io/badge/Rust-2021-f74c00?style=flat-square">
    <img alt="macOS" src="https://img.shields.io/badge/macOS-Keychain-111111?style=flat-square">
    <img alt="Local First" src="https://img.shields.io/badge/local--first-credential%20broker-2f855a?style=flat-square">
    <img alt="License" src="https://img.shields.io/badge/license-MIT-blue?style=flat-square">
  </p>
  <p>
    <a href="README.md">English</a>
    · <a href="#快速开始">快速开始</a>
    · <a href="#http-api">HTTP API</a>
    · <a href="#开发">开发</a>
  </p>
</div>

Passka 的定位很简单：它是一个本地密码管理器，同时带一层给 AI 用的临时访问 Broker。

用户把长期凭证保存在本机的 macOS Keychain 里。AI 不会直接拿到这些明文凭证，而是先申请一个短时 `Lease`，再通过 Passka 的 HTTP 代理或占位符替换能力去访问上游服务。

如果上游密钥泄露，你只需要在 Passka 里轮换凭证，不需要改 AI 侧的接入方式。

## 核心模型

| 概念 | 含义 |
| --- | --- |
| Credential Account | 一份已保存的凭证账号，可以是 API Key、OAuth、OTP 或 opaque secret。 |
| Agent | 被允许使用某个账号的本地 AI 工具或自动化。 |
| Lease | 某个账号的短时访问票据。 |
| Access Method | 用 HTTP 代理，或者在代理时做占位符替换。 |
| Audit | 对账号授权、拒绝、代理、刷新、查看等行为的审计记录。 |

## 为什么这样设计

如果不给 Broker，最常见的做法就是把 API Key 放进环境变量或者直接复制给 AI。这样虽然方便，但边界很脆弱。

Passka 把风险尽量留在本机：

1. 你通过 macOS App 或 CLI 添加一个凭证账号。
2. 长期 secret 存进 macOS Keychain。
3. 你授权某个本地 agent 可以使用这个账号。
4. agent 为该账号申请一个短期 lease。
5. agent 带着 lease 走 Passka 的代理或占位符替换能力。
6. Passka 把过程写进审计日志。

## 安全边界

- 长期凭证保存在 macOS Keychain，服务名是 `passka-broker`。
- Broker 状态保存在 `~/.config/passka/broker/state.json`。
- CLI 可以添加、列出、查看元数据、授权账号、申请 lease、代理请求。
- CLI 不提供明文 secret reveal。
- macOS App 是唯一的人类 secret reveal 入口，并且会先做本地认证。
- AI 拿到的是 lease 和代理结果，不是原始 API Key 或 refresh token。

## 安装

GitHub Releases 会发布按架构区分的 macOS 制品，分别给 CLI 和桌面 App 使用：

- `passka-cli-<version>-macos-x86_64.tar.gz`
- `passka-cli-<version>-macos-arm64.tar.gz`
- `Passka-<version>-macos-x86_64.zip`
- `Passka-<version>-macos-arm64.zip`

请按你的 Mac 架构选择对应文件。

安装 CLI：

```bash
tar -xzf passka-cli-<version>-macos-<arch>.tar.gz
mkdir -p "$HOME/.local/bin"
mv passka "$HOME/.local/bin/passka"
chmod +x "$HOME/.local/bin/passka"
```

如果 `~/.local/bin` 还没加到 `PATH`，可以在 `~/.zshrc` 里加入：

```bash
export PATH="$HOME/.local/bin:$PATH"
```

安装 macOS App：

1. 解压 `Passka-<version>-macos-<arch>.zip`
2. 把 `Passka.app` 拖到 `/Applications`
3. 第一次打开如果被 macOS 提示“未签名”，右键应用并选择 `Open`

当前 release 制品还没有签名和 notarize，所以首次启动需要手动确认一次 `Open`。

## 快速开始

启动本地 broker：

```bash
cargo run -p passka-cli -- broker serve --addr 127.0.0.1:8478
```

检查健康状态：

```bash
curl http://127.0.0.1:8478/health
```

添加一个账号：

```bash
cargo run -p passka-cli -- account add openai-prod \
  --provider openai \
  --auth api_key \
  --base-url https://api.openai.com
```

先看一下已有 principal：

```bash
cargo run -p passka-cli -- principal list
```

Passka 默认会自动创建一个本地 agent：`principal:local-agent`。如果你想注册新的 agent：

```bash
cargo run -p passka-cli -- principal add my-agent \
  --kind agent \
  --description "My local AI agent"
```

授权默认本地 agent 使用这个账号：

```bash
cargo run -p passka-cli -- account allow <account_id> \
  --agent principal:local-agent \
  --lease-seconds 300
```

申请一个短时 lease：

```bash
cargo run -p passka-cli -- request \
  --principal principal:local-agent \
  --account <account_id> \
  --environment local \
  --purpose "model discovery"
```

带着 lease 走代理：

```bash
cargo run -p passka-cli -- proxy \
  --lease <lease_id> \
  --method GET \
  --path https://api.openai.com/v1/models
```

查看审计记录：

```bash
cargo run -p passka-cli -- audit list --limit 20
```

## 支持的凭证类型

Passka 用统一的“账号”模型承载不同类型的凭证：

- `api_key`：API Key 加认证 header 元数据
- `oauth`：本地完成授权、本地刷新，但不会把 refresh token 给 agent
- `otp`：保存 TOTP seed，供人类在 macOS App 中查看
- `opaque`：任意 key-value secret 集合

这些只是凭证类型，不是不同的产品架构。

## 访问方式

Passka 支持两种使用 lease 的方式。

### 1. HTTP 代理

agent 把目标请求发给 Passka，Passka 在本地注入真实凭证后再发给上游。

直接代理接口：

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

普通 forward proxy：

```bash
curl -x http://127.0.0.1:8478 \
  --proxy-header "X-Passka-Lease: <lease_id>" \
  https://api.openai.com/v1/models
```

### 2. 代理时做占位符替换

Passka 可以在转发前，替换 header 或文本 body 里的占位符。

主账号可用占位符：

- `PASSKA_API_KEY`
  只用于主账号是 API Key 凭证的场景。
- `PASSKA_TOKEN`
  只用于主账号是 OAuth access token 的场景。

额外账号可以绑定 alias：

```bash
cargo run -p passka-cli -- proxy \
  --lease <openai_lease_id> \
  --extra-lease github=<github_lease_id> \
  --extra-lease slack=<slack_lease_id> \
  --method POST \
  --path https://api.example.test/composite \
  --body '{"openai":"PASSKA_API_KEY","github":"PASSKA_GITHUB_API_KEY","slack":"PASSKA_SLACK_TOKEN","github_account":"PASSKA_GITHUB_ACCOUNT_ID"}'
```

别名占位符遵循 `PASSKA_${NAME}_${FIELD}` 规则。常见例子：

- `PASSKA_GITHUB_API_KEY`
- `PASSKA_SLACK_TOKEN`
- `PASSKA_GITHUB_ACCOUNT_ID`

## macOS App

macOS App 是给人类使用的凭证管理前台：

- 浏览已保存的凭证账号
- 添加 API Key、OAuth、OTP 和 opaque 账号
- 注册 agent principal
- 给 agent 授权可用账号
- 通过本地认证后查看敏感字段
- 查看某个账号最近的审计活动

构建命令：

```bash
cd app && swift build
```

## HTTP API

`passka broker serve` 提供本地 daemon，给 agent、MCP bridge 和 App 使用：

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

说明：

- `/accounts/{account_id}/authorize` 用来把某个 agent principal 绑定到一个账号。
- `/authorizations` 用来查看当前账号到 agent 的授权关系。
- `/access/request` 现在以 `principal_id + account_id` 为主。
- `/app/accounts/{account_id}/reveal` 是给 macOS App 用的专用入口，不是通用 agent API。

## 开发

构建 Rust workspace：

```bash
cargo build
```

运行 Rust 测试：

```bash
cargo test --workspace
```

构建 macOS App：

```bash
cd app && swift build
```

发布 GitHub Release 的方式也很简单：推送一个类似 `v0.1.0` 的语义化版本 tag。release workflow 会分别构建 macOS Intel 和 Apple Silicon 制品，打包 CLI 与桌面 App，并把它们连同 `SHA256SUMS.txt` 一起上传到 GitHub Release。
