# 2026-03-04 main 新变更分析与 Rust 适配

## 背景
- 基线：`rust` 分支历史基于 `d5e0499e`
- 新主干：`origin/main` 已推进到 `022ebb56`
- 已执行：`rust` 合并 `origin/main`（merge commit: `121c68a1`）

## main 侧关键变更（与 API/前端联调相关）
1. Providers API 语义调整（Python）
- `ProviderInfo` 移除 `has_api_key`
- `set_active_model` 校验逻辑从 `is_configured()` 转为按 provider 类型分支校验：
  - custom: 必须有 `base_url`
  - ollama: 必须有 `base_url`
  - 非 local 且非上述：必须有 `api_key`

2. Providers 持久化路径与安全策略（Python）
- `providers.json` 迁移到 secret 目录（`COPAW_SECRET_DIR`）
- 增加 legacy 路径迁移
- 处理“路径是目录”异常，避免 500

3. Envs 持久化路径与迁移（Python）
- `envs.json` 迁移到 secret 目录（`COPAW_SECRET_DIR`）
- 增加 legacy 路径迁移
- 增加“路径是目录”保护

4. Provider 模型列表更新（Python）
- OpenAI/Azure 列表从 `gpt-5-chat` 转向 `gpt-5.2`/`gpt-5`

## 本轮 Rust 适配内容

### A. Providers 语义对齐
- 文件：`crates/providers/src/store.rs`
- 改动：`ProvidersData::is_configured()` 与 Python 对齐
  - local => true
  - ollama => 仅当 settings 中 `base_url` 非空
  - custom => `effective_base_url` 非空
  - built-in remote => `providers` 中存在配置项即可

### B. `set_active_model` 校验逻辑对齐
- 文件：`crates/app/src/routes/models.rs`
- 改动：不再只依赖 `is_configured()`，改为按 provider 类型显式校验，行为与 Python 一致

### C. ProviderInfo 字段对齐
- 文件：`crates/app/src/routes/schemas.rs`
- 改动：移除 `has_api_key` 字段
- 联动更新：
  - `crates/app/src/routes/models.rs`
  - `crates/app/src/routes/local_models.rs`
  - `crates/app/src/routes/ollama_models.rs`

### D. providers.json 路径与迁移
- 文件：`crates/providers/src/store.rs`
- 改动：
  - 默认路径改为 `COPAW_SECRET_DIR/providers.json`
  - 支持 legacy 路径迁移：
    - `src/copaw/providers/providers.json`
    - `${COPAW_WORKING_DIR}/providers.json`
  - 处理“目标路径存在但不是普通文件”错误

### E. envs.json 路径与迁移
- 文件：`crates/app/src/envs/store.rs`
- 改动：
  - 默认路径改为 `COPAW_SECRET_DIR/envs.json`
  - 支持 legacy 路径迁移：
    - `src/copaw/envs/envs.json`
    - `${COPAW_WORKING_DIR}/envs.json`
  - 处理“目标路径存在但是目录”的保护逻辑

### F. Provider 模型列表对齐
- 文件：`crates/providers/src/registry.rs`
- 改动：OpenAI/Azure 模型列表新增 `gpt-5.2` / `gpt-5`，不再以 `gpt-5-chat` 为首选

### G. 依赖更新
- 文件：
  - `crates/app/Cargo.toml`
  - `crates/providers/Cargo.toml`
  - `Cargo.lock`
- 改动：新增 `dirs` 依赖用于路径展开/目录推导

## 验证结果
- `cargo test -p copaw-providers --quiet test_is_configured` 通过
- `cargo test -p copaw --quiet test_set_active_model_empty_model` 通过
- `cargo test -p copaw --quiet test_env_store_save_and_load` 通过

> 说明：仓库当前存在较多历史 warning（unused imports/dead code），本轮未做 warning 清理，以避免扩大变更面。

## 下一步建议
1. 执行一次接口回归（重点页面）
- `/agent-config`
- `/mcp`
- `/heartbeat`
- `/cron-jobs`
- `/settings/models`

2. 执行契约测试
- `cargo test -p copaw --test python_rust_api_contract_test`

3. 若前端仍有“Unexpected API response”，优先抓取对应 endpoint 的实际 JSON 与 Python 对照差异。
