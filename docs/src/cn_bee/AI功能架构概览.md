# Zed AI 功能架构概览

## 四层 AI 能力

| 层级 | 功能 | 定位 |
|------|------|------|
| Agent Panel | 完整的 Agent 工作面板 | 核心。可调用工具、读写文件、执行命令 |
| Inline Assistant | 选中代码后提示转换 | 轻量编辑。选中 → 描述 → 替换 |
| Edit Prediction | 按键级代码补全 | 自动预测下一步编辑（类似 Copilot） |
| Text Threads | 纯文本对话 | 早期功能，无工具调用，纯问答 |

## LLM 提供商（14 个）

**三种接入方式，可混用：**
1. **Zed 托管模型** — 需订阅（Pro $5/月），价格 = 供应商价 + 10%
2. **自带 API Key** — 无需订阅，直接填 key
3. **外部 Agent（ACP 协议）** — 无需订阅

**支持的供应商：**
- Anthropic（Claude 系列）
- OpenAI（GPT 系列）
- Google AI（Gemini）
- Amazon Bedrock
- DeepSeek
- GitHub Copilot Chat
- LM Studio（本地）
- Mistral
- Ollama（本地/远程）
- OpenAI 兼容接口（Together AI、Anyscale 等）
- OpenRouter
- Vercel AI Gateway
- Vercel v0
- xAI（Grok）

**配置方式：** `settings.json` 中设置 provider + model name，API key 存在系统钥匙链中。

## 内置工具（14 个）

### 读取类
| 工具 | 用途 |
|------|------|
| `diagnostics` | 获取文件/项目的错误和警告 |
| `fetch` | 下载 URL 内容转 Markdown |
| `find_path` | glob 模式查找文件 |
| `grep` | 正则搜索代码 |
| `list_directory` | 列出目录内容 |
| `now` | 获取当前时间 |
| `open` | 用默认程序打开文件/URL |
| `read_file` | 读取文件内容 |
| `thinking` | 推理但不执行 |
| `web_search` | 联网搜索 |

### 编辑类
| 工具 | 用途 |
|------|------|
| `copy_path` | 递归复制文件/目录 |
| `create_directory` | 创建目录 |
| `delete_path` | 删除文件/目录 |
| `edit_file` | 替换指定文本范围 |
| `move_path` | 移动/重命名 |
| `restore_file_from_disk` | 从磁盘重新加载 |
| `save_file` | 保存未保存的修改 |
| `terminal` | 执行 shell 命令 |

### 其他
| 工具 | 用途 |
|------|------|
| `spawn_agent` | 创建子 Agent 并行调查 |

## 工具权限系统

优先级从高到低：
1. **内置安全规则**（硬编码，如禁止 `rm -rf /`，不可覆盖）
2. `always_deny` 模式匹配
3. `always_confirm` 模式匹配
4. `always_allow` 模式匹配
5. 工具级 `default`
6. 全局 `default`

支持正则匹配，可按工具、按路径模式精细控制。

## ACP 协议与外部 Agent

**Agent Client Protocol (ACP)** 是 Zed 集成外部 Agent 的协议。

**内置支持的外部 Agent：**

| Agent | 底层 | 认证方式 | MCP 支持 |
|-------|------|----------|----------|
| Gemini CLI | Google Gemini | Google 账号 / Vertex AI | 不支持 |
| Claude Agent | Claude Code（ACP 适配） | 独立认证（与 Zed 的 Anthropic key 解耦） | 支持 |
| Codex CLI | OpenAI Codex | ChatGPT 订阅 / API Key | 支持 |

**外部 Agent 的限制：**
- 不支持编辑已发送的消息
- 不支持恢复历史会话
- 不支持 checkpoint 回滚

**添加自定义 Agent：**
- 通过 ACP Registry（推荐）
- 通过 settings.json 配置 `command` + `args` + `env`
- 调试：`dev: open acp logs` 查看协议消息

## MCP（Model Context Protocol）

开放协议，用于连接 LLM 到外部工具和数据源。

**安装方式：**
1. 作为扩展安装（从 Zed 扩展市场）
2. 作为自定义服务器配置（JSON 指定 command/URL/headers）
3. 远程服务器（支持 OAuth）

**权限控制：** `mcp:<server>:<tool_name>` 格式

## Rules 系统（项目指令）

自动注入到 Agent 交互中的提示词。

**文件查找优先级（第一个命中即停）：**
1. `.rules`
2. `.cursorrules`
3. `.windsurfrules`
4. `.clinerules`
5. `.github/copilot-instructions.md`
6. `AGENT.md` / `AGENTS.md`
7. `CLAUDE.md`
8. `GEMINI.md`

Rules Library 支持创建、复用、设为默认规则。

## 隐私与数据

- 默认零数据保留，所有供应商均承诺不用于训练
- 反馈评分 = 选择性数据共享（整个对话线程会被存储）
- Edit Prediction (Zeta) 默认不收集训练数据，可选择性为开源项目贡献
- 自动排除敏感文件（.env, .pem, .key, .cert 等）

## 关闭 AI

```json
{ "disable_ai": true }
```
