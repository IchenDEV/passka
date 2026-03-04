# passka

AI Agent 友好的凭据管理工具。以 macOS Keychain 为后端，支持 4 种凭据类型，核心特色是 `exec` 注入模式 — 凭据不进入 AI 上下文。

## 安装

```bash
cargo install --path crates/passka-cli
```

安装后 `passka` 命令可用。

## 支持的凭据类型

| 类型 | 字段 | 用途 |
|------|------|------|
| `api_key` | key, secret?, endpoint? | API 密钥（支持 AK/SK 双密钥） |
| `password` | username, password, url? | 用户名密码对 |
| `session` | domain + 动态 KV 对 | 浏览器 Cookie / Header / Session |
| `oauth` | authorize_url, token_url, client_id, client_secret, ... | OAuth 协议（支持自动刷新） |

## CLI 使用

### 添加凭据

```bash
passka add openai --type api_key
# 引导式输入：key → 是否有 secret → endpoint

passka add github --type password --description "GitHub account"
# 引导式输入：username → password → URL

passka add jira --type session --description "Jira session"
# 引导式输入：domain → 循环添加 header/cookie KV 对

passka add slack --type oauth --description "Slack workspace"
# 引导式输入：authorize_url → token_url → client_id → client_secret → redirect_uri → scopes
```

### OAuth 授权

OAuth 凭据需要两步完成：

```bash
passka add slack --type oauth          # 第一步：配置端点和客户端凭证
passka auth slack                      # 第二步：浏览器授权流程（自动启动本地回调服务器）
```

### 带凭据执行命令（推荐）

```bash
# 单凭据注入
passka exec openai -- curl -s \
  -H "Authorization: Bearer $OPENAI_API_KEY" \
  https://api.openai.com/v1/models

# 多凭据同时注入
passka exec openai github -- python pipeline.py

# 输出脱敏（凭据值替换为 [REDACTED]）
passka exec --redact openai -- python debug_api.py
```

### 查看和管理

```bash
passka list                    # 列出所有凭据（元数据）
passka list --type api_key     # 按类型过滤
passka show openai             # 显示详情（敏感字段脱敏）
passka update openai --field key  # 更新字段
passka rm openai               # 删除
passka refresh slack           # 刷新 OAuth Token
```

## 隐私模型

`passka exec` 是唯一的凭据访问方式：

- 凭据注入为子进程的环境变量，不经过 stdout
- `--redact` 标志捕获并替换输出中的敏感值为 `[REDACTED]`
- `show` 命令只展示脱敏后的值
- `refresh` 命令不输出 token 值

AI Skill 硬约束确保 Agent 只通过 `exec` 访问凭据，禁止生成打印敏感值的代码。

## macOS App

SwiftUI 原生应用，提供图形界面管理凭据：

- 三栏布局：类型侧边栏 → 凭据列表 → 详情面板
- Touch ID 指纹验证后才能查看真实值
- 值显示 30 秒后自动隐藏
- 复制到剪贴板 60 秒后自动清除

构建 App：

```bash
cargo build --release -p passka-ffi
cd app && swift build
```

## AI Agent 集成

安装 Cursor Skill 后，AI 会自动使用 `passka exec` 模式访问凭据，确保密钥不进入对话上下文。

```bash
# AI 生成的典型命令
passka exec openai -- curl -s -H "Authorization: Bearer $OPENAI_API_KEY" https://api.openai.com/v1/models

# 多凭据场景
passka exec openai github -- python deploy.py

# 调试时使用 --redact
passka exec --redact openai -- python test_auth.py
```

## 存储架构

- **元数据索引**: `~/.config/passka/index.json`
- **敏感数据**: macOS Keychain（service: `passka`）

## License

MIT
