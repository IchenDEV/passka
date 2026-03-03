# passka

AI Agent 友好的凭据管理工具。以 macOS Keychain 为后端，支持多种凭据类型，核心特色是 `exec` 注入模式 — 凭据不进入 AI 上下文。

## 安装

```bash
cargo install --path crates/passka-cli
```

安装后 `passka` 命令可用。

## 支持的凭据类型

| 类型 | 字段 | 用途 |
|------|------|------|
| `api_key` | key, provider?, endpoint? | LLM API 密钥 |
| `user_pass` | username, password, url? | 网站登录 |
| `cookie` | value, domain, path?, expires? | 浏览器会话 |
| `app_secret` | access_key, secret_key, app_name? | 飞书、AWS 等 |
| `token` | token, refresh_token?, expires_at?, ... | OAuth/Bearer Token |
| `custom` | 自定义 key-value | 其他 |

## CLI 使用

### 添加凭据

```bash
passka add openai --type api_key
# 交互式输入，密码不回显

passka add github --type user_pass --description "GitHub account"
```

### 获取凭据值

```bash
# 获取主字段
passka get openai

# 获取指定字段
passka get github --field password
```

### 带凭据执行命令（推荐，隐私安全）

```bash
# 凭据注入为环境变量，执行子命令
passka exec openai -- curl -s \
  -H "Authorization: Bearer $OPENAI_API_KEY" \
  https://api.openai.com/v1/models

# 在脚本中使用
passka exec github -- python deploy.py
```

### 查看和管理

```bash
passka list                    # 列出所有凭据（元数据）
passka list --type api_key     # 按类型过滤
passka show openai             # 显示详情（值脱敏）
passka update openai --field key  # 更新字段
passka rm openai               # 删除
passka refresh my-oauth-token  # 刷新 OAuth Token
```

### 代码片段生成

```bash
passka snippet openai --lang bash
passka snippet openai --lang python
passka snippet openai --lang javascript
```

### 环境变量导出

```bash
passka env openai
# 输出: export OPENAI_API_KEY="$(passka get openai --field key)"
```

## 隐私模型

三层访问模式（隐私从高到低）：

1. **`passka exec`**（推荐）: 凭据注入子进程环境变量，AI 只看到命令输出
2. **Shell 替换** `$(passka get ...)`: 凭据由 shell 展开
3. **`passka get`**: 直接输出到 stdout，慎用

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
```

## 存储架构

- **元数据索引**: `~/.config/passka/index.json`
- **敏感数据**: macOS Keychain（service: `passka`）

## License

MIT
