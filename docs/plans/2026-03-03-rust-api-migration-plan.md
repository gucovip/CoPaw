# CoPaw Python -> Rust API Migration Plan

**Date**: 2026-03-03  
**Objective**: Fully migrate visible Python FastAPI endpoints to Rust Axum while keeping API contract stable for console/frontend.

## Baseline

| Item | Value |
|---|---:|
| Python visible routes | 78 |
| Rust registered routes | 13 |
| Exact matched | 9 |
| Semantic matched | 2 |
| Missing in Rust | 67 |

## Migration Principles

1. Keep response payloads and HTTP status codes backward compatible with current Python APIs.
2. Prefer exact path parity; remove semantic mismatches (`:param` vs `{param}`) before GA.
3. Each route migration must include tests and tracker update in the same PR.
4. Maintain rollback path until all critical flows are verified in production-like env.

## Phases

### Phase 0: Contract Freeze and Tooling (0.5 day)
1. Freeze Python route contract and lock this tracker format.
2. Add CI check to detect route drift between Python and Rust.
3. Define Done criteria for each route (handler + tests + docs + tracker).

### Phase 1: Config Domain Completion (1 day)
1. Align parameter naming with Python (`/api/config/channels/{channel_name}`).
2. Validate `GET/PUT /api/config` behavior and error contract parity.
3. Add integration tests for full config lifecycle.

### Phase 2: Low-Risk Utilities (1-2 days)
1. `/api/envs` (GET/PUT/DELETE).
2. `/api/workspace/download`, `/api/workspace/upload`.
3. `/api/console/push-messages`.

### Phase 3: Model Management (3-4 days)
1. `/api/models/*` provider/model management and active model flows.
2. `/api/local-models/*` download/status/cancel/delete.
3. `/api/ollama-models/*` parity implementation.

### Phase 4: Skills and MCP (3 days)
1. `/api/skills/*` list/install/enable-disable/file-read APIs.
2. `/api/mcp/*` CRUD + toggle APIs.
3. End-to-end validation from console UI.

### Phase 5: Core Runtime APIs (4-5 days)
1. `/api/chats/*` conversation lifecycle APIs.
2. `/api/cron/*` scheduler APIs (jobs, pause/resume/run/state).
3. `/api/agent/*` files/memory/running-config APIs.

### Phase 6: Hardening and Cutover (1-2 days)
1. Full regression and contract tests.
2. Staged rollout with fallback to Python service.
3. Remove fallback after stable observation window.

## Prioritized Backlog by Business Impact

1. P0: `config`, `models`, `chats` (blocks core console usage).
2. P1: `skills`, `mcp`, `cron` (automation and orchestration).
3. P2: `local-models`, `ollama-models`, `envs`, `workspace`, `console`.
4. P3: root/static parity refinements and cleanup.

## Definition of Done (Per Route)

1. Rust route registered with exact method/path contract.
2. Request/response schema and status codes aligned with Python behavior.
3. Unit + integration tests added and passing.
4. Tracker updated (`exact` / `semantic` / `missing`).
5. Change log entry added for migrated domain.

## Coordination Model

1. Stream A: `config`, `envs`, `workspace`, `console`.
2. Stream B: `models`, `local-models`, `ollama-models`.
3. Stream C: `skills`, `mcp`.
4. Stream D: `chats`, `cron`, `agent`.

