# CoPaw Python -> Rust API 迁移实施计划

**日期**: 2026-03-03
**范围**: 78 个 Python API 端点迁移到 Rust

---

## 概述

本计划将 78 个 Python API 端点迁移到 Rust，按优先级和依赖关系分为 6 个阶段。迁移利用现有的 Rust 基础设施（259 个测试通过），并保持与 Python API 的完全向后兼容。

---

## 当前状态分析

### Python API 汇总（78 个端点）

| 路由 | 端点数 | 描述 |
|-------|---------|------|
| `/api/version` | 1 | 版本信息（Rust 已完成） |
| `/api/config/*` | 7 | 渠道和心跳配置 |
| `/api/models/*` | 10 | 提供商/模型管理 |
| `/api/chats/*` | 6 | 聊天历史管理 |
| `/api/channels/*` | 6 | 渠道操作（通过配置） |
| `/api/cron/*` | 9 | 定时任务管理 |
| `/api/skills/*` | 11 | Skills 管理和市场 |
| `/api/agent/*` | 8 | Agent 文件/内存管理 |
| `/api/mcp/*` | 6 | MCP 客户端管理 |
| `/api/envs/*` | 3 | 环境变量 |
| `/api/local-models/*` | 5 | 本地模型下载 |
| `/api/ollama-models/*` | 5 | Ollama 模型管理 |
| `/api/workspace/*` | 2 | 工作区导出/导入 |
| `/api/console/*` | 1 | 推送消息 |

### 现有 Rust 基础设施

- **crates/core**: Agent, Tool, Memory, LLMProvider, Channel, Message traits
- **crates/agents**: ReActAgent 实现
- **crates/providers**: OpenAIProvider, LlamaCppProvider, OllamaProvider
- **crates/channels**: FeishuChannel 实现
- **crates/tools**: ShellTool, FileTool
- **crates/app**: 带 version 端点的基础 Axum 服务器

---

## 阶段 1: 基础 - 配置系统（第 1 周）

**目标**: 建立所有其他模块依赖的配置加载/保存基础设施。

### 1.1 Config Crate 设置

**创建新的 crate 结构:**
```
crates/config/
  src/
    lib.rs
    config.rs       # 主配置模型
    channels.rs     # 渠道配置
    heartbeat.rs    # 心跳配置
    providers.rs    # 提供商设置
    mcp.rs         # MCP 客户端配置
    watcher.rs     # 热重载监视器
```

### 1.2 核心配置模型 (config.rs)

- **操作**: 创建匹配 Python config.json 模式的 `CoPawConfig` 结构
- **原因**: 所有端点依赖配置加载
- **依赖**: 无
- **风险**: 低
- **时间**: 2 小时

**关键结构:**
```rust
pub struct CoPawConfig {
    pub channels: ChannelConfig,
    pub agents: AgentsConfig,
    pub mcp: MCPConfig,
}

pub struct AgentsConfig {
    pub defaults: AgentDefaults,
    pub running: AgentsRunningConfig,
}
```

### 1.3 配置加载/保存 (lib.rs)

- **操作**: 实现 `load_config()`, `save_config()`, `get_config_path()`
- **原因**: 所有配置相关端点需要
- **依赖**: 1.2
- **风险**: 低
- **时间**: 1.5 小时

### 1.4 渠道配置模型 (channels.rs)

- **操作**: 创建所有渠道配置结构（Feishu, DingTalk, Discord 等）
- **原因**: `/api/config/channels` 端点需要
- **依赖**: 无
- **风险**: 低
- **时间**: 2 小时

### 1.5 心跳配置 (heartbeat.rs)

- **操作**: 创建带 ActiveHours 支持的 HeartbeatConfig
- **原因**: `/api/config/heartbeat` 端点需要
- **依赖**: 无
- **风险**: 低
- **时间**: 1 小时

### 1.6 配置 API 路由 (routes/config.rs)

- **操作**: 实现所有 7 个配置端点
- **原因**: 高优先级，阻塞其他功能
- **依赖**: 1.2-1.5
- **风险**: 中

**端点:**
1. `GET /api/config/channels` - 列出所有渠道
2. `GET /api/config/channels/types` - 列出渠道类型
3. `PUT /api/config/channels` - 更新所有渠道
4. `GET /api/config/channels/{name}` - 获取单个渠道
5. `PUT /api/config/channels/{name}` - 更新单个渠道
6. `GET /api/config/heartbeat` - 获取心跳配置
7. `PUT /api/config/heartbeat` - 更新心跳配置

**时间**: 3 小时

---

## 阶段 2: 提供商/模型管理（第 2 周）

**目标**: 完成模型提供商管理系统。

### 2.1 提供商注册表 (providers/src/registry.rs)

- **操作**: 创建带内置/自定义提供商的注册表
- **原因**: 提供商定义的中心位置
- **依赖**: 阶段 1
- **风险**: 中
- **时间**: 2 小时

### 2.2 提供商存储 (providers/src/store.rs)

- **操作**: 实现 providers.json 加载/保存
- **原因**: 持久化提供商设置和 API 密钥
- **依赖**: 2.1
- **风险**: 低
- **时间**: 1.5 小时

### 2.3 提供商 API 模型 (routes/schemas/models.rs)

- **操作**: 创建匹配 Python 的请求/响应模型
- **原因**: API 契约兼容性
- **依赖**: 无
- **风险**: 低
- **时间**: 1 小时

### 2.4 提供商 API 路由 (routes/models.rs)

- **操作**: 实现所有 10 个提供商端点
- **原因**: 模型配置的关键
- **依赖**: 2.1-2.3
- **风险**: 中

**端点:**
1. `GET /api/models` - 列出所有提供商
2. `PUT /api/models/{id}/config` - 配置提供商
3. `POST /api/models/custom-providers` - 创建自定义提供商
4. `DELETE /api/models/custom-providers/{id}` - 删除自定义提供商
5. `POST /api/models/{id}/test` - 测试提供商连接
6. `POST /api/models/{id}/models/test` - 测试特定模型
7. `POST /api/models/{id}/models` - 添加模型到提供商
8. `DELETE /api/models/{id}/models/{model_id}` - 删除模型
9. `GET /api/models/active` - 获取活跃 LLM
10. `PUT /api/models/active` - 设置活跃 LLM

**时间**: 4 小时

### 2.5 连接测试 (providers/src/testing.rs)

- **操作**: 实现提供商/模型连接测试
- **原因**: 验证 API 密钥和端点
- **依赖**: 2.4
- **风险**: 中（外部 API 调用）
- **时间**: 2 小时

---

## 阶段 3: 聊天和 Agent 管理（第 3 周）

**目标**: 启用聊天历史和 Agent 文件管理。

### 3.1 聊天存储库 (repo/chat_repo.rs)

- **操作**: 创建基于 JSON 的聊天存储库
- **原因**: 持久化聊天元数据
- **依赖**: 阶段 1
- **风险**: 低
- **时间**: 2 小时

### 3.2 聊天管理器 (runner/chat_manager.rs)

- **操作**: 实现带 CRUD 操作的 ChatManager
- **原因**: 聊天管理的业务逻辑
- **依赖**: 3.1
- **风险**: 低
- **时间**: 2 小时

### 3.3 聊天 API 路由 (routes/chats.rs)

- **操作**: 实现所有 6 个聊天端点
- **原因**: 用户聊天历史管理
- **依赖**: 3.1-3.2
- **风险**: 中

**端点:**
1. `GET /api/chats` - 列出聊天（带过滤器）
2. `POST /api/chats` - 创建聊天
3. `POST /api/chats/batch-delete` - 批量删除
4. `GET /api/chats/{id}` - 获取聊天历史
5. `PUT /api/chats/{id}` - 更新聊天
6. `DELETE /api/chats/{id}` - 删除聊天

**时间**: 3 小时

### 3.4 Agent 文件管理器 (agents/src/file_manager.rs)

- **操作**: 创建 working_dir/memory markdown 文件管理器
- **原因**: 支持 Agent 文件操作
- **依赖**: 无
- **风险**: 低
- **时间**: 2 小时

### 3.5 Agent API 路由 (routes/agent.rs)

- **操作**: 实现所有 8 个 Agent 端点
- **原因**: 工作文件和内存管理
- **依赖**: 3.4
- **风险**: 低

**端点:**
1. `GET /api/agent/files` - 列出工作文件
2. `GET /api/agent/files/{name}` - 读取工作文件
3. `PUT /api/agent/files/{name}` - 写入工作文件
4. `GET /api/agent/memory` - 列出内存文件
5. `GET /api/agent/memory/{name}` - 读取内存文件
6. `PUT /api/agent/memory/{name}` - 写入内存文件
7. `GET /api/agent/running-config` - 获取运行配置
8. `PUT /api/agent/running-config` - 更新运行配置

**时间**: 2.5 小时

---

## 阶段 4: Cron 和渠道管理（第 4 周）

**目标**: 启用定时任务和渠道操作。

### 4.1 任务存储库 (crons/job_repo.rs)

- **操作**: 创建基于 JSON 的任务存储库
- **原因**: 持久化 Cron 任务定义
- **依赖**: 阶段 1
- **风险**: 低
- **时间**: 1.5 小时

### 4.2 任务模型 (crons/models.rs)

- **操作**: 创建 CronJobSpec, CronJobState, CronJobView
- **原因**: 匹配 Python 模式
- **依赖**: 无
- **风险**: 低
- **时间**: 1 小时

### 4.3 Cron 管理器 (crons/manager.rs)

- **操作**: 实现带调度器的 CronManager
- **原因**: 执行定时任务
- **依赖**: 4.1-4.2
- **风险**: 高（异步调度）
- **时间**: 4 小时

### 4.4 Cron API 路由 (routes/cron.rs)

- **操作**: 实现所有 9 个 Cron 端点
- **原因**: 任务管理 UI
- **依赖**: 4.1-4.3
- **风险**: 中

**端点:**
1. `GET /api/cron/jobs` - 列出任务
2. `POST /api/cron/jobs` - 创建任务
3. `GET /api/cron/jobs/{id}` - 获取任务详情
4. `PUT /api/cron/jobs/{id}` - 更新任务
5. `DELETE /api/cron/jobs/{id}` - 删除任务
6. `POST /api/cron/jobs/{id}/pause` - 暂停任务
7. `POST /api/cron/jobs/{id}/resume` - 恢复任务
8. `POST /api/cron/jobs/{id}/run` - 立即运行
9. `GET /api/cron/jobs/{id}/state` - 获取任务状态

**时间**: 3 小时

### 4.5 渠道管理器 (channels/manager.rs)

- **操作**: 实现 ChannelManager 用于启动/停止所有
- **原因**: 编排多个渠道
- **依赖**: crates/channels
- **风险**: 中
- **时间**: 3 小时

---

## 阶段 5: Skills, MCP, 环境（第 5 周）

**目标**: 完成中间件功能。

### 5.1 Skill 服务 (skills/service.rs)

- **操作**: 创建 Skill 管理服务
- **原因**: 处理 Skill CRUD 操作
- **依赖**: 阶段 1
- **风险**: 中
- **时间**: 3 小时

### 5.2 Skills Hub 客户端 (skills/hub.rs)

- **操作**: 实现 Hub 搜索和安装
- **原因**: 外部 Skill 市场
- **依赖**: 5.1
- **风险**: 中（HTTP 客户端）
- **时间**: 2 小时

### 5.3 Skills API 路由 (routes/skills.rs)

- **操作**: 实现所有 11 个 Skills 端点
- **原因**: Skill 管理 UI
- **依赖**: 5.1-5.2
- **风险**: 中

**端点:**
1. `GET /api/skills` - 列出所有 Skills
2. `GET /api/skills/available` - 列出可用的
3. `GET /api/skills/hub/search` - 搜索 Hub
4. `POST /api/skills/hub/install` - 从 Hub 安装
5. `POST /api/skills` - 创建 Skill
6. `POST /api/skills/batch-enable` - 批量启用
7. `POST /api/skills/batch-disable` - 批量禁用
8. `POST /api/skills/{name}/enable` - 启用 Skill
9. `POST /api/skills/{name}/disable` - 禁用 Skill
10. `DELETE /api/skills/{name}` - 删除 Skill
11. `GET /api/skills/{name}/files/{source}/{path}` - 加载文件

**时间**: 4 小时

### 5.4 MCP 客户端管理器 (mcp/manager.rs)

- **操作**: 实现 MCP 客户端生命周期
- **原因**: 外部工具提供商
- **依赖**: 阶段 1
- **风险**: 高（MCP 协议）
- **时间**: 4 小时

### 5.5 MCP API 路由 (routes/mcp.rs)

- **操作**: 实现所有 6 个 MCP 端点
- **原因**: MCP 配置 UI
- **依赖**: 5.4
- **风险**: 中

**端点:**
1. `GET /api/mcp` - 列出 MCP 客户端
2. `GET /api/mcp/{key}` - 获取客户端
3. `POST /api/mcp` - 创建客户端
4. `PUT /api/mcp/{key}` - 更新客户端
5. `PATCH /api/mcp/{key}/toggle` - 切换客户端
6. `DELETE /api/mcp/{key}` - 删除客户端

**时间**: 2.5 小时

### 5.6 环境变量 (routes/envs.rs)

- **操作**: 实现所有 3 个环境变量端点
- **原因**: 安全环境管理
- **依赖**: 阶段 1
- **风险**: 低

**端点:**
1. `GET /api/envs` - 列出环境变量
2. `PUT /api/envs` - 批量保存环境变量
3. `DELETE /api/envs/{key}` - 删除环境变量

**时间**: 1.5 小时

---

## 阶段 6: 模型下载和工作区（第 6 周）

**目标**: 完成剩余功能。

### 6.1 下载任务存储 (download/task_store.rs)

- **操作**: 创建异步下载任务管理器
- **原因**: 跟踪后台下载
- **依赖**: 无
- **风险**: 中（异步状态）
- **时间**: 2.5 小时

### 6.2 本地模型服务 (local_models/service.rs)

- **操作**: 包装 llama.cpp/MLX 模型管理
- **原因**: 本地模型下载
- **依赖**: 6.1
- **风险**: 高（外部二进制）
- **时间**: 4 小时

### 6.3 本地模型 API 路由 (routes/local_models.rs)

- **操作**: 实现所有 5 个本地模型端点
- **原因**: 模型下载 UI
- **依赖**: 6.1-6.2
- **风险**: 中

**端点:**
1. `GET /api/local-models` - 列出已下载的模型
2. `POST /api/local-models/download` - 开始下载
3. `GET /api/local-models/download-status` - 获取状态
4. `DELETE /api/local-models/{id}` - 删除模型
5. `POST /api/local-models/cancel-download/{id}` - 取消下载

**时间**: 3 小时

### 6.4 Ollama 模型服务 (ollama/service.rs)

- **操作**: 实现 Ollama API 客户端
- **原因**: Ollama 模型管理
- **依赖**: 6.1
- **风险**: 中（外部服务）
- **时间**: 3 小时

### 6.5 Ollama 模型 API 路由 (routes/ollama_models.rs)

- **操作**: 实现所有 5 个 Ollama 模型端点
- **原因**: Ollama 管理 UI
- **依赖**: 6.4
- **风险**: 中

**端点:**
1. `GET /api/ollama-models` - 列出 Ollama 模型
2. `POST /api/ollama-models/download` - 拉取模型
3. `GET /api/ollama-models/download-status` - 获取状态
4. `DELETE /api/ollama-models/download/{id}` - 取消下载
5. `DELETE /api/ollama-models/{name}` - 删除模型

**时间**: 2.5 小时

### 6.6 工作区 API 路由 (routes/workspace.rs)

- **操作**: 实现工作区 zip 导出/导入
- **原因**: 备份/恢复功能
- **依赖**: 无
- **风险**: 中（文件操作）

**端点:**
1. `GET /api/workspace/download` - 下载工作区 zip
2. `POST /api/workspace/upload` - 上传并合并 zip

**时间**: 3 小时

### 6.7 Console 推送消息 (routes/console.rs)

- **操作**: 实现推送消息存储
- **原因**: 实时通知
- **依赖**: 无
- **风险**: 低

**端点:**
1. `GET /api/console/push-messages` - 获取推送消息

**时间**: 1.5 小时

---

## 架构变更汇总

### 新 Crate: crates/config/

```
crates/config/
  Cargo.toml
  src/
    lib.rs           # 公共导出
    config.rs        # CoPawConfig, AgentsConfig
    channels.rs      # ChannelConfig 变体
    heartbeat.rs     # HeartbeatConfig, ActiveHours
    mcp.rs         # MCPClientConfig
    watcher.rs       # 热重载文件监视器
    utils.rs         # get_config_path 等
```

### App Crate 重构: crates/app/

```
crates/app/
  Cargo.toml
  src/
    main.rs          # 带路由设置的入口点
    routes/
      mod.rs
      config.rs      # 阶段 1
      models.rs      # 阶段 2
      chats.rs       # 阶段 3
      agent.rs       # 阶段 3
      cron.rs        # 阶段 4
      skills.rs      # 阶段 5
      mcp.rs         # 阶段 5
      envs.rs        # 阶段 5
      local_models.rs # 阶段 6
      ollama_models.rs # 阶段 6
      workspace.rs   # 阶段 6
      console.rs     # 阶段 6
      schemas/
        mod.rs
        config.rs
        models.rs
        chat.rs
        cron.rs
        skill.rs
        mcp.rs
    repo/
      mod.rs
      chat_repo.rs
      job_repo.rs
    crons/
      mod.rs
      models.rs
      manager.rs
    channels/
      mod.rs
      manager.rs
    skills/
      mod.rs
      service.rs
      hub.rs
    mcp/
      mod.rs
      manager.rs
    download/
      mod.rs
      task_store.rs
    local_models/
      mod.rs
      service.rs
    ollama/
      mod.rs
      service.rs
    console_push_store.rs
    state.rs         # AppState 定义
    error.rs         # AppError 和处理器
```

---

## 测试策略

### 单元测试
- 隔离的每个路由处理器
- 配置加载/保存
- 存储库 CRUD 操作
- 管理器业务逻辑

### 集成测试
- 完整的 API 请求/响应循环
- 状态持久化（JSON 文件）
- 错误处理场景

### E2E 测试
- 使用现有 Playwright 测试对抗 Rust 服务器
- 与 Python API 响应的兼容性验证

---

## 风险和缓解

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| 异步调度器复杂度 | 高 | 使用 tokio-cron-scheduler crate |
| MCP 协议变更 | 高 | 固定到稳定 MCP 版本 |
| 外部服务依赖 | 中 | 在测试中模拟服务 |
| 配置模式漂移 | 中 | 从 Rust 类型生成模式 |
| 文件系统竞态条件 | 中 | 使用带适当锁定的 tokio::fs |
| 下载取消 | 中 | 实现适当的任务取消 |

---

## 成功标准

- [ ] 所有 78 个端点已实现和测试
- [ ] API 响应与 Python 格式完全匹配
- [ ] 所有现有 Playwright 测试通过
- [ ] 新代码 80%+ 代码覆盖率
- [ ] 配置热重载工作
- [ ] 下载任务可以取消
- [ ] MCP 客户端可以在运行时切换
- [ ] 工作区导出/导入保留所有文件

---

## 时间估算汇总

| 阶段 | 时长 | 累计 |
|-------|------|------|
| 阶段 1: 配置 | 10.5 小时 | 第 1 周 |
| 阶段 2: 提供商 | 10.5 小时 | 第 2 周 |
| 阶段 3: 聊天/Agent | 11.5 小时 | 第 3 周 |
| 阶段 4: Cron/渠道 | 12.5 小时 | 第 4 周 |
| 阶段 5: Skills/MCP/环境 | 17.5 小时 | 第 5 周 |
| 阶段 6: 下载/工作区 | 17 小时 | 第 6 周 |

**总估算时间**: 约 80 小时，6 周

---

## 各阶段内推荐实施顺序

1. **模型优先**: 创建请求/响应模式
2. **存储库**: 实现数据持久化
3. **服务/管理器**: 业务逻辑
4. **路由**: HTTP 处理器
5. **测试**: 验证与 Python API
