# R-001 侦察报告：合并基线 + 全量 Diff

## 任务概述
创建干净的 squash commit，包含 `taiji-quant` 分支上所有对 upstream 的自定义改动。

## Squash Commit
- **Hash**: `2d0599e60`
- **消息**: `feat: taiji-quant all customizations (squashed)`
- **基线**: `upstream/main`（merge-base: `44178470f`）
- **分支**: `baseline-clean`

## 统计摘要
- **文件数**: 499
- **新增行数**: +69,756
- **删除行数**: -3,446
- **Squash 提交数**: 1（`upstream/main..baseline-clean`）

## 文件清单

### Root 构建与配置
- `.github/CODEOWNERS`
- `.gitignore`
- `Cargo.toml`
- `package.json`
- `pnpm-lock.yaml`
- `pnpm-workspace.yaml`
- `CHANGELOG.md`
- `PR-BODY.md`
- `ACKNOWLEDGMENTS.md`
- `CODE_OF_CONDUCT.md`

### Workbuddy / AI 辅助
- `.workbuddy/HANDBOOK.md`
- `.workbuddy/gbrain-start.ps1`
- `.workbuddy/memory/2026-07-26.md`
- `.workbuddy/memory/MEMORY.md`

### 脚本
- `scripts/cargo-target-gc.mjs`
- `scripts/dev.cjs`
- `scripts/embed-server.py`
- `scripts/gen-plan.py`
- `scripts/sync-upstream.ps1`
- `scripts/test-onnx.py`

### Installer（BitFun-Installer）
- `BitFun-Installer/src-tauri/Cargo.toml`
- `BitFun-Installer/src-tauri/tauri.conf.json`
- `BitFun-Installer/src/taiji-icon.png`

### 讨论记录（discussion-group/）
包含 30+ 份讨论方案、互评、立场收敛文档：
- `discussion-group/00-会议召集.md`
- `discussion-group/00-第一次汇报汇总.md`
- `discussion-group/01-方案-01.md`
- `discussion-group/02-回应-05.md`, `02-回应-06.md`, `02-回应-08.md`, `02-回应-09.md`
- `discussion-group/02-方案-02.md`
- `discussion-group/03-方案-03.md`
- `discussion-group/04-方案-04.md`
- `discussion-group/05-互评-02/03/06/07/09.md`
- `discussion-group/05-方案-05.md`, `05-方案-05-v2.md`
- `discussion-group/06-方案-06.md`
- `discussion-group/07-方案-07.md`
- `discussion-group/08-方案-08.md`
- `discussion-group/09-回应-01~10.md`（含审查/互评/交叉质疑）
- `discussion-group/09-方案-09.md`, `09-方案-09-v2.md`
- `discussion-group/10-方案-10.md`
- `discussion-group/final-solution.md`
- 立场收敛文档：`02-立场-07分歧收敛.md`, `02-立场-核心分歧.md`, `09-立场-分歧收敛.md`, `09-最终立场-第三轮.md`
- 其他：`06-通知-方案已提交.md`, `09-通知-已提交.md`, `09-回应-双模式.md`, `09-回应-审计日志轮换.md`

### 文档（docs/）
- `docs/code-map-taiji-quant.md`（量化代码地图）
- `docs/taiji-quant-change-summary.md`（变更总结）
- 中文规范：`Git提交规范.md`, `Rust工具链配置建议.md`, `代码审查标准_Checklist.md`, `代码审查流程文档.md`, `技术债务治理策略.md`

#### 知识库（docs/knowledge-base/）
- `docs/knowledge-base/index.md`
- `docs/knowledge-base/dependency-graph.md`
- `docs/knowledge-base/crates/bitfun/` — 22 篇每个 bitfun crate 的文档
- `docs/knowledge-base/crates/taiji/` — 18 篇每个 taiji crate 的文档
- `docs/knowledge-base/features/agentic-system.md`, `frontend-architecture.md`, `legion-mode.md`, `quant-engine.md`, `ultra-mode.md`
- `docs/knowledge-base/references/bug-index.md`, `data-flows.md`

#### 法律
- `docs/legal/upstream-authorization.md`

#### 规划（docs/plans/）
- `docs/plans/phase-rbac-poke-dispatch-prompts.md`
- `docs/plans/phase-rbac-poke-plan.md`
- `docs/plans/phase-rbac-poke-type-contract.md`
- `docs/plans/product-cli-mcp-acp-dispatch.md`
- `docs/plans/product-cli-mcp-acp-plan.md`
- `docs/plans/product-cli-mcp-acp-types.md`
- `docs/plans/r003-audit-v1.1.md`
- `docs/plans/r003-audit.md`
- `docs/plans/r003-design.md`
- `docs/plans/r003-dispatch.md`
- `docs/plans/r003-handoff.md`
- `docs/plans/r003-requirements.md`
- `docs/plans/r004-design.md`
- `docs/plans/r004-dispatch.md`
- `docs/plans/r004-progress-final.md`
- `docs/plans/r004-requirements.md`

### 量化 Crate 基座（src/crates/taiji/）
- `src/crates/taiji/LOGGING.md`
- `src/crates/taiji/THIRD_PARTY_NOTICES.md`
- `src/crates/taiji/product.toml`, `product.free.toml`, `product.standard.toml`, `product.ultimate.toml`

#### taiji-abnormal
`src/crates/taiji/taiji-abnormal/` — Cargo.toml, README.md, src/ 全部 7 文件

#### taiji-agents
`src/crates/taiji/taiji-agents/` — 8 篇 agent 设计文档

#### taiji-alert
`src/crates/taiji/taiji-alert/` — Cargo.toml, README.md, src/ 全部 3 文件

#### taiji-backtest
`src/crates/taiji/taiji-backtest/` — Cargo.toml, README.md, src/ 6 文件

#### taiji-bar
`src/crates/taiji/taiji-bar/` — Cargo.toml, README.md, src/lib.rs

#### taiji-blog-gen
`src/crates/taiji/taiji-blog-gen/` — Cargo.toml, README.md, src/main.rs, templates/ 3, test_data/ 1

#### taiji-cli
`src/crates/taiji/taiji-cli/` — Cargo.toml, README.md, src/ 6 文件

#### taiji-content
`src/crates/taiji/taiji-content/` — Cargo.toml, README.md, src/ 8 文件, src/types/ 6 文件

#### taiji-engine
`src/crates/taiji/taiji-engine/` — Cargo.toml, README.md, src/ 20+ 文件, tests/ 5, benches/ 1, config/

#### taiji-engine-py
`src/crates/taiji/taiji-engine-py/` — Cargo.toml, README.md, pyproject.toml, src/ 8 文件

#### taiji-example
`src/crates/taiji/taiji-example/` — Cargo.toml, README.md, src/lib.rs

#### taiji-executor
`src/crates/taiji/taiji-executor/` — Cargo.toml, README.md, src/ 5 文件

#### taiji-growth
`src/crates/taiji/taiji-growth/` — Cargo.toml, README.md, src/ 7 文件, templates/ 5

#### taiji-knowledge-graph
`src/crates/taiji/taiji-knowledge-graph/` — Cargo.toml, README.md, build.rs, src/ 3 文件

#### taiji-llm
`src/crates/taiji/taiji-llm/` — Cargo.toml, README.md, src/ 7 文件

#### taiji-orderflow
`src/crates/taiji/taiji-orderflow/` — Cargo.toml, README.md, src/ 4 文件

#### taiji-pattern
`src/crates/taiji/taiji-pattern/` — Cargo.toml, README.md, src/ 4 文件

#### taiji-publisher
`src/crates/taiji/taiji-publisher/` — AGENTS.md, Cargo.toml, README.md, src/ 7 文件

#### taiji-realtime
`src/crates/taiji/taiji-realtime/` — Cargo.toml, README.md, src/ 4 文件

#### taiji-sentiment
`src/crates/taiji/taiji-sentiment/` — Cargo.toml, README.md, config/, src/ 4 文件

#### taiji-strategen
`src/crates/taiji/taiji-strategen/` — Cargo.toml, README.md, src/ 7 文件

#### taiji-strategy-template
`src/crates/taiji/taiji-strategy-template/` — Cargo.toml, README.md, src/lib.rs

### 上游修改（Assembly Core + Contracts + Execution + Services）
修改了以下上游 crate 中的既有文件：
- `src/crates/assembly/core/` — agents/registry/（builtin, external, mod）, coordination/（6 文件）, events/types, goal_mode, memories, persistence, session/session_manager, tools/implementations/（9 文件）, tools/pipeline/（2 文件）, tools/restrictions, tools/tool_context_runtime, warden/（4 文件）, function_agents, service/config, service/session_usage, service_agent_runtime, tests/
- `src/crates/contracts/core-types/` — lib, session, session_tree
- `src/crates/contracts/events/` — agentic, frontend_projection
- `src/crates/contracts/runtime-ports/` — lib
- `src/crates/execution/agent-runtime/` — runtime, sdk, session, session_control, tests/
- `src/crates/execution/tool-contracts/` — framework, lib, poke, tests/
- `src/crates/execution/tool-execution/` — context
- `src/crates/interfaces/acp/` — Cargo.toml, client/manager
- `src/crates/services/services-core/` — session/（6 文件）
- `src/crates/services/services-integrations/` — function_agents, mcp/protocol（2 文件）

### 桌面与 CLI 集成
- `src/apps/desktop/icons/taiji-*.png`（8 图标）
- `src/apps/desktop/src/api/browser_api.rs`
- `src/apps/cli/src/agent/runtime_client.rs`
- `src/apps/cli/src/daemon/service.rs`
- `src/apps/cli/src/peer_host/commands/session.rs`
- `src/apps/cli/src/self_update.rs`

### Web UI 前端
- `src/web-ui/index.html`, `preview.html`
- `src/web-ui/public/taiji-icon-128.png`, `taiji-icon.png`
- `src/web-ui/src/app/components/NavPanel/sections/sessions/`（scss, tsx）
- `src/web-ui/src/app/layout/BeeColonyMonitor.tsx`
- `src/web-ui/src/app/scenes/agents/components/CreateLegionPage.tsx`, `LegionCard.tsx`
- `src/web-ui/src/app/scenes/agents/data/orchestration-patterns.ts`
- `src/web-ui/src/app/startup/startupPerformanceContract.test.ts`
- `src/web-ui/src/flow_chat/` — 11 文件（components, hooks, services, store, types, utils）
- `src/web-ui/src/infrastructure/api/service-api/AgentAPI.ts`, `LegionPresetAPI.ts`
- `src/web-ui/src/infrastructure/config/components/` — 20+ 文件（ModelSelect, ReviewConfig, SkillsConfig, WorktreesConfig, form-controls, common, subscriptionLoginCoordinator）
- `src/web-ui/src/locales/*/flow-chat.json`（3 语言）
- `src/web-ui/src/shared/types/session-history.ts`

## 验证
- baseline-clean 是基于 `upstream/main`（merge-base `44178470f`）创建的纯净基线
- 仅包含一个 squash commit: `2d0599e60`
- `git diff upstream/main baseline-clean --stat` 验证 499 文件，+69,756 / -3,446
