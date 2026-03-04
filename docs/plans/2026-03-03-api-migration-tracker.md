# CoPaw API Migration Tracker (Round 11)

**Date**: 2026-03-03  
**Scope**: Python FastAPI routes visible in this repo vs Rust Axum routes mounted in `crates/app/src/main.rs`

## Summary

| Metric | Value |
|---|---:|
| Python visible routes | 78 |
| Rust mounted routes | 96 |
| Exact matched | 42 |
| Semantic matched | 35 |
| Missing in Rust (exact) | 36 |
| Missing in Rust (semantic) | 1 |

## Progress by Domain

| Domain | Total | Exact | Semantic | Missing |
|---|---:|---:|---:|---:|
| `/api/agent` | 8 | 4 | 4 | 0 |
| `/api/chats` | 6 | 3 | 3 | 0 |
| `/api/config` | 7 | 5 | 2 | 0 |
| `/api/console` | 1 | 1 | 0 | 0 |
| `/api/cron` | 9 | 2 | 7 | 0 |
| `/api/envs` | 3 | 2 | 1 | 0 |
| `/api/local-models` | 5 | 3 | 2 | 0 |
| `/api/mcp` | 6 | 2 | 4 | 0 |
| `/api/models` | 10 | 4 | 6 | 0 |
| `/api/ollama-models` | 5 | 3 | 2 | 0 |
| `/api/skills` | 11 | 7 | 4 | 0 |
| `/api/version` | 1 | 1 | 0 | 0 |
| `/api/workspace` | 2 | 2 | 0 | 0 |
| `root/static` | 4 | 3 | 0 | 1 |

## Route-Level Tracker

Status legend: `exact` = method+path exact match; `semantic` = route pattern aligns but syntax/name differs; `missing` = no mounted Rust route found.

| # | Method | Path | Domain | Status | Python Source | Notes |
|---:|---|---|---|---|---|---|
| 1 | GET | `/api/agent/files` | `/api/agent` | `exact` | `src/copaw/app/routers/agent.py` |  |
| 2 | GET | `/api/agent/files/{md_name}` | `/api/agent` | `semantic` | `src/copaw/app/routers/agent.py` | Path pattern equivalent but syntax/name differs. |
| 3 | PUT | `/api/agent/files/{md_name}` | `/api/agent` | `semantic` | `src/copaw/app/routers/agent.py` | Path pattern equivalent but syntax/name differs. |
| 4 | GET | `/api/agent/memory` | `/api/agent` | `exact` | `src/copaw/app/routers/agent.py` |  |
| 5 | GET | `/api/agent/memory/{md_name}` | `/api/agent` | `semantic` | `src/copaw/app/routers/agent.py` | Path pattern equivalent but syntax/name differs. |
| 6 | PUT | `/api/agent/memory/{md_name}` | `/api/agent` | `semantic` | `src/copaw/app/routers/agent.py` | Path pattern equivalent but syntax/name differs. |
| 7 | GET | `/api/agent/running-config` | `/api/agent` | `exact` | `src/copaw/app/routers/agent.py` |  |
| 8 | PUT | `/api/agent/running-config` | `/api/agent` | `exact` | `src/copaw/app/routers/agent.py` |  |
| 9 | GET | `/api/chats` | `/api/chats` | `exact` | `src/copaw/app/runner/api.py` |  |
| 10 | POST | `/api/chats` | `/api/chats` | `exact` | `src/copaw/app/runner/api.py` |  |
| 11 | POST | `/api/chats/batch-delete` | `/api/chats` | `exact` | `src/copaw/app/runner/api.py` |  |
| 12 | DELETE | `/api/chats/{chat_id}` | `/api/chats` | `semantic` | `src/copaw/app/runner/api.py` | Path pattern equivalent but syntax/name differs. |
| 13 | GET | `/api/chats/{chat_id}` | `/api/chats` | `semantic` | `src/copaw/app/runner/api.py` | Path pattern equivalent but syntax/name differs. |
| 14 | PUT | `/api/chats/{chat_id}` | `/api/chats` | `semantic` | `src/copaw/app/runner/api.py` | Path pattern equivalent but syntax/name differs. |
| 15 | GET | `/api/config/channels` | `/api/config` | `exact` | `src/copaw/app/routers/config.py` |  |
| 16 | PUT | `/api/config/channels` | `/api/config` | `exact` | `src/copaw/app/routers/config.py` |  |
| 17 | GET | `/api/config/channels/types` | `/api/config` | `exact` | `src/copaw/app/routers/config.py` |  |
| 18 | GET | `/api/config/channels/{channel_name}` | `/api/config` | `semantic` | `src/copaw/app/routers/config.py` | Path pattern equivalent but syntax/name differs. |
| 19 | PUT | `/api/config/channels/{channel_name}` | `/api/config` | `semantic` | `src/copaw/app/routers/config.py` | Path pattern equivalent but syntax/name differs. |
| 20 | GET | `/api/config/heartbeat` | `/api/config` | `exact` | `src/copaw/app/routers/config.py` |  |
| 21 | PUT | `/api/config/heartbeat` | `/api/config` | `exact` | `src/copaw/app/routers/config.py` |  |
| 22 | GET | `/api/console/push-messages` | `/api/console` | `exact` | `src/copaw/app/routers/console.py` |  |
| 23 | GET | `/api/cron/jobs` | `/api/cron` | `exact` | `src/copaw/app/crons/api.py` |  |
| 24 | POST | `/api/cron/jobs` | `/api/cron` | `exact` | `src/copaw/app/crons/api.py` |  |
| 25 | DELETE | `/api/cron/jobs/{job_id}` | `/api/cron` | `semantic` | `src/copaw/app/crons/api.py` | Path pattern equivalent but syntax/name differs. |
| 26 | GET | `/api/cron/jobs/{job_id}` | `/api/cron` | `semantic` | `src/copaw/app/crons/api.py` | Path pattern equivalent but syntax/name differs. |
| 27 | PUT | `/api/cron/jobs/{job_id}` | `/api/cron` | `semantic` | `src/copaw/app/crons/api.py` | Path pattern equivalent but syntax/name differs. |
| 28 | POST | `/api/cron/jobs/{job_id}/pause` | `/api/cron` | `semantic` | `src/copaw/app/crons/api.py` | Path pattern equivalent but syntax/name differs. |
| 29 | POST | `/api/cron/jobs/{job_id}/resume` | `/api/cron` | `semantic` | `src/copaw/app/crons/api.py` | Path pattern equivalent but syntax/name differs. |
| 30 | POST | `/api/cron/jobs/{job_id}/run` | `/api/cron` | `semantic` | `src/copaw/app/crons/api.py` | Path pattern equivalent but syntax/name differs. |
| 31 | GET | `/api/cron/jobs/{job_id}/state` | `/api/cron` | `semantic` | `src/copaw/app/crons/api.py` | Path pattern equivalent but syntax/name differs. |
| 32 | GET | `/api/envs` | `/api/envs` | `exact` | `src/copaw/app/routers/envs.py` |  |
| 33 | PUT | `/api/envs` | `/api/envs` | `exact` | `src/copaw/app/routers/envs.py` |  |
| 34 | DELETE | `/api/envs/{key}` | `/api/envs` | `semantic` | `src/copaw/app/routers/envs.py` | Path pattern equivalent but syntax/name differs. |
| 35 | GET | `/api/local-models` | `/api/local-models` | `exact` | `src/copaw/app/routers/local_models.py` |  |
| 36 | POST | `/api/local-models/cancel-download/{task_id}` | `/api/local-models` | `semantic` | `src/copaw/app/routers/local_models.py` | Path pattern equivalent but syntax/name differs. |
| 37 | POST | `/api/local-models/download` | `/api/local-models` | `exact` | `src/copaw/app/routers/local_models.py` |  |
| 38 | GET | `/api/local-models/download-status` | `/api/local-models` | `exact` | `src/copaw/app/routers/local_models.py` |  |
| 39 | DELETE | `/api/local-models/{model_id:path}` | `/api/local-models` | `semantic` | `src/copaw/app/routers/local_models.py` | Path pattern equivalent but syntax/name differs. |
| 40 | GET | `/api/mcp` | `/api/mcp` | `exact` | `src/copaw/app/routers/mcp.py` |  |
| 41 | POST | `/api/mcp` | `/api/mcp` | `exact` | `src/copaw/app/routers/mcp.py` |  |
| 42 | DELETE | `/api/mcp/{client_key}` | `/api/mcp` | `semantic` | `src/copaw/app/routers/mcp.py` | Path pattern equivalent but syntax/name differs. |
| 43 | GET | `/api/mcp/{client_key}` | `/api/mcp` | `semantic` | `src/copaw/app/routers/mcp.py` | Path pattern equivalent but syntax/name differs. |
| 44 | PUT | `/api/mcp/{client_key}` | `/api/mcp` | `semantic` | `src/copaw/app/routers/mcp.py` | Path pattern equivalent but syntax/name differs. |
| 45 | PATCH | `/api/mcp/{client_key}/toggle` | `/api/mcp` | `semantic` | `src/copaw/app/routers/mcp.py` | Path pattern equivalent but syntax/name differs. |
| 46 | GET | `/api/models` | `/api/models` | `exact` | `src/copaw/app/routers/providers.py` |  |
| 47 | GET | `/api/models/active` | `/api/models` | `exact` | `src/copaw/app/routers/providers.py` |  |
| 48 | PUT | `/api/models/active` | `/api/models` | `exact` | `src/copaw/app/routers/providers.py` |  |
| 49 | POST | `/api/models/custom-providers` | `/api/models` | `exact` | `src/copaw/app/routers/providers.py` |  |
| 50 | DELETE | `/api/models/custom-providers/{provider_id}` | `/api/models` | `semantic` | `src/copaw/app/routers/providers.py` | Path pattern equivalent but syntax/name differs. |
| 51 | PUT | `/api/models/{provider_id}/config` | `/api/models` | `semantic` | `src/copaw/app/routers/providers.py` | Path pattern equivalent but syntax/name differs. |
| 52 | POST | `/api/models/{provider_id}/models` | `/api/models` | `semantic` | `src/copaw/app/routers/providers.py` | Path pattern equivalent but syntax/name differs. |
| 53 | POST | `/api/models/{provider_id}/models/test` | `/api/models` | `semantic` | `src/copaw/app/routers/providers.py` | Path pattern equivalent but syntax/name differs. |
| 54 | DELETE | `/api/models/{provider_id}/models/{model_id:path}` | `/api/models` | `semantic` | `src/copaw/app/routers/providers.py` | Path pattern equivalent but syntax/name differs. |
| 55 | POST | `/api/models/{provider_id}/test` | `/api/models` | `semantic` | `src/copaw/app/routers/providers.py` | Path pattern equivalent but syntax/name differs. |
| 56 | GET | `/api/ollama-models` | `/api/ollama-models` | `exact` | `src/copaw/app/routers/ollama_models.py` |  |
| 57 | POST | `/api/ollama-models/download` | `/api/ollama-models` | `exact` | `src/copaw/app/routers/ollama_models.py` |  |
| 58 | GET | `/api/ollama-models/download-status` | `/api/ollama-models` | `exact` | `src/copaw/app/routers/ollama_models.py` |  |
| 59 | DELETE | `/api/ollama-models/download/{task_id}` | `/api/ollama-models` | `semantic` | `src/copaw/app/routers/ollama_models.py` | Path pattern equivalent but syntax/name differs. |
| 60 | DELETE | `/api/ollama-models/{name:path}` | `/api/ollama-models` | `semantic` | `src/copaw/app/routers/ollama_models.py` | Path pattern equivalent but syntax/name differs. |
| 61 | GET | `/api/skills` | `/api/skills` | `exact` | `src/copaw/app/routers/skills.py` |  |
| 62 | POST | `/api/skills` | `/api/skills` | `exact` | `src/copaw/app/routers/skills.py` |  |
| 63 | GET | `/api/skills/available` | `/api/skills` | `exact` | `src/copaw/app/routers/skills.py` |  |
| 64 | POST | `/api/skills/batch-disable` | `/api/skills` | `exact` | `src/copaw/app/routers/skills.py` |  |
| 65 | POST | `/api/skills/batch-enable` | `/api/skills` | `exact` | `src/copaw/app/routers/skills.py` |  |
| 66 | POST | `/api/skills/hub/install` | `/api/skills` | `exact` | `src/copaw/app/routers/skills.py` |  |
| 67 | GET | `/api/skills/hub/search` | `/api/skills` | `exact` | `src/copaw/app/routers/skills.py` |  |
| 68 | DELETE | `/api/skills/{skill_name}` | `/api/skills` | `semantic` | `src/copaw/app/routers/skills.py` | Path pattern equivalent but syntax/name differs. |
| 69 | POST | `/api/skills/{skill_name}/disable` | `/api/skills` | `semantic` | `src/copaw/app/routers/skills.py` | Path pattern equivalent but syntax/name differs. |
| 70 | POST | `/api/skills/{skill_name}/enable` | `/api/skills` | `semantic` | `src/copaw/app/routers/skills.py` | Path pattern equivalent but syntax/name differs. |
| 71 | GET | `/api/skills/{skill_name}/files/{source}/{file_path:path}` | `/api/skills` | `semantic` | `src/copaw/app/routers/skills.py` | Path pattern equivalent but syntax/name differs. |
| 72 | GET | `/api/version` | `/api/version` | `exact` | `src/copaw/app/_app.py` |  |
| 73 | GET | `/api/workspace/download` | `/api/workspace` | `exact` | `src/copaw/app/routers/workspace.py` |  |
| 74 | POST | `/api/workspace/upload` | `/api/workspace` | `exact` | `src/copaw/app/routers/workspace.py` |  |
| 75 | GET | `/` | `root/static` | `exact` | `src/copaw/app/_app.py` |  |
| 76 | GET | `/copaw-symbol.svg` | `root/static` | `exact` | `src/copaw/app/_app.py` |  |
| 77 | GET | `/logo.png` | `root/static` | `exact` | `src/copaw/app/_app.py` |  |
| 78 | GET | `/{full_path:path}` | `root/static` | `missing` | `src/copaw/app/_app.py` |  |

## Rust-Only Mounted Routes (No Exact Python Counterpart)

| Method | Path | Rust Source |
|---|---|---|
| GET | `/api/agent/files/:md_name` | `crates/app/src/routes/agent.rs` |
| PUT | `/api/agent/files/:md_name` | `crates/app/src/routes/agent.rs` |
| GET | `/api/agent/memory/:md_name` | `crates/app/src/routes/agent.rs` |
| PUT | `/api/agent/memory/:md_name` | `crates/app/src/routes/agent.rs` |
| DELETE | `/api/chats/:chat_id` | `crates/app/src/routes/chats.rs` |
| GET | `/api/chats/:chat_id` | `crates/app/src/routes/chats.rs` |
| PUT | `/api/chats/:chat_id` | `crates/app/src/routes/chats.rs` |
| GET | `/api/config` | `crates/app/src/routes/config.rs` |
| PUT | `/api/config` | `crates/app/src/routes/config.rs` |
| GET | `/api/config/channels/:channel_name` | `crates/app/src/routes/config.rs` |
| PUT | `/api/config/channels/:channel_name` | `crates/app/src/routes/config.rs` |
| DELETE | `/api/cron/jobs/:job_id` | `crates/app/src/routes/cron.rs` |
| GET | `/api/cron/jobs/:job_id` | `crates/app/src/routes/cron.rs` |
| PUT | `/api/cron/jobs/:job_id` | `crates/app/src/routes/cron.rs` |
| POST | `/api/cron/jobs/:job_id/pause` | `crates/app/src/routes/cron.rs` |
| POST | `/api/cron/jobs/:job_id/resume` | `crates/app/src/routes/cron.rs` |
| POST | `/api/cron/jobs/:job_id/run` | `crates/app/src/routes/cron.rs` |
| GET | `/api/cron/jobs/:job_id/state` | `crates/app/src/routes/cron.rs` |
| POST | `/api/envs` | `crates/app/src/routes/envs.rs` |
| DELETE | `/api/envs/:key` | `crates/app/src/routes/envs.rs` |
| DELETE | `/api/local-models/*model_id` | `crates/app/src/routes/local_models.rs` |
| POST | `/api/local-models/cancel-download/:task_id` | `crates/app/src/routes/local_models.rs` |
| GET | `/api/local-models/providers` | `crates/app/src/routes/local_models.rs` |
| POST | `/api/local-models/providers/:provider/download` | `crates/app/src/routes/local_models.rs` |
| GET | `/api/local-models/providers/:provider/models` | `crates/app/src/routes/local_models.rs` |
| DELETE | `/api/local-models/providers/:provider/models/:model` | `crates/app/src/routes/local_models.rs` |
| GET | `/api/local-models/providers/:provider/models/:model/info` | `crates/app/src/routes/local_models.rs` |
| DELETE | `/api/mcp/:client_key` | `crates/app/src/routes/mcp.rs` |
| GET | `/api/mcp/:client_key` | `crates/app/src/routes/mcp.rs` |
| PUT | `/api/mcp/:client_key` | `crates/app/src/routes/mcp.rs` |
| POST | `/api/mcp/:client_key/test` | `crates/app/src/routes/mcp.rs` |
| PATCH | `/api/mcp/:client_key/toggle` | `crates/app/src/routes/mcp.rs` |
| PUT | `/api/models/:provider_id/config` | `crates/app/src/routes/models.rs` |
| POST | `/api/models/:provider_id/models` | `crates/app/src/routes/models.rs` |
| DELETE | `/api/models/:provider_id/models/:model_id` | `crates/app/src/routes/models.rs` |
| POST | `/api/models/:provider_id/models/test` | `crates/app/src/routes/models.rs` |
| POST | `/api/models/:provider_id/test` | `crates/app/src/routes/models.rs` |
| DELETE | `/api/models/custom-providers/:provider_id` | `crates/app/src/routes/models.rs` |
| DELETE | `/api/ollama-models/*name` | `crates/app/src/routes/ollama_models.rs` |
| DELETE | `/api/ollama-models/download/:task_id` | `crates/app/src/routes/ollama_models.rs` |
| GET | `/api/ollama-models/providers` | `crates/app/src/routes/ollama_models.rs` |
| GET | `/api/ollama-models/providers/:provider/models` | `crates/app/src/routes/ollama_models.rs` |
| DELETE | `/api/ollama-models/providers/:provider/models/:model` | `crates/app/src/routes/ollama_models.rs` |
| GET | `/api/ollama-models/providers/:provider/models/:model/info` | `crates/app/src/routes/ollama_models.rs` |
| POST | `/api/ollama-models/providers/:provider/pull` | `crates/app/src/routes/ollama_models.rs` |
| DELETE | `/api/skills/:skill_name` | `crates/app/src/routes/skills.rs` |
| GET | `/api/skills/:skill_name/config` | `crates/app/src/routes/skills.rs` |
| PUT | `/api/skills/:skill_name/config` | `crates/app/src/routes/skills.rs` |
| POST | `/api/skills/:skill_name/disable` | `crates/app/src/routes/skills.rs` |
| POST | `/api/skills/:skill_name/enable` | `crates/app/src/routes/skills.rs` |
| GET | `/api/skills/:skill_name/files/:source/*file_path` | `crates/app/src/routes/skills.rs` |
| GET | `/api/skills/hub` | `crates/app/src/routes/skills.rs` |
| GET | `/api/skills/installed` | `crates/app/src/routes/skills.rs` |
| GET | `/api/skills/market` | `crates/app/src/routes/skills.rs` |
