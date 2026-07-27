# R-ID 矩阵：功能 PR 拆分规划

> 生成日期：2026-07-28  
> 上次验证：2026-07-28（本次任务）  
> 数据来源：r001-diff-files.txt（499 变更文件）、r002-reference-index.md（参考材料索引）、r003-gbrain-insights.md（已知错误列表）、_recon_category_map.md（功能分类映射）  
> 总文件数：499 | 总新增行数：+69,756 | 总删除行数：-3,446  
> **验证状态**: `git diff upstream/main baseline-clean --stat` 确认 499 文件 | `--numstat` 确认 +69,756 / −3,446 | 文件清单已与 `--name-only` 输出交叉核对 ✅

---

## 概览

| 波次 | R-ID | PR 名称 | 文件数 | 预估行数 (新增/删除) | 依赖 | 优先级 |
|------|------|---------|:------:|:--------------------:|:----:|:------:|
| 0 | R-0.1 | 工作空间与构建配置 | ~12 | +500 / −30 | — | P2 |
| 0 | R-0.2 | 量化 crate 基座与产品定义 | ~10 | +800 / −20 | R-0.1 | P2 |
| 1 | R-1.1 | L3 128K Token 限制修复 | ~3 | +50 / −5 | — | **P0** |
| 1 | R-1.2 | Cancelled 状态映射修复 | ~2 | +30 / −5 | — | **P0** |
| 1 | R-1.3 | Windows 兼容性修复 (F1) | ~8 | +400 / −50 | — | **P0** |
| 1 | R-1.4 | 前端 Bug 修复集 (F2) | ~12 | +800 / −100 | — | **P0** |
| 1 | R-1.5 | Task Spawn 与 Workspace 路径修复 | ~4 | +80 / −15 | — | P1 |
| 2 | R-2.1 | R-003 Session 事件与上下文修复 (B1) | ~12 | +2,000 / −120 | — | **P0** |
| 2 | R-2.2 | R-004 树形拓扑与级联删除 (B2) | ~15 | +2,500 / −150 | R-2.1 | **P0** |
| 2 | R-2.3 | IPC 协议增强 (B4) | ~8 | +1,200 / −80 | R-2.2 | P2 |
| 3 | R-3.1 | Session 树前端 UI 组件 (B3) | ~10 | +2,000 / −100 | R-2.2 | **P0** |
| 4 | R-4.1 | taiji-bar 与数据源适配层 | ~12 | +2,500 / −50 | — | P1 |
| 4 | R-4.2 | Pipeline 管线与状态管理 | ~12 | +3,000 / −80 | R-4.1 | P1 |
| 4 | R-4.3 | DAG 引擎、信号融合与权重校准 | ~15 | +3,500 / −100 | R-4.1 | P1 |
| 4 | R-4.4 | 多智能体辩论系统 (Debate) | ~10 | +3,000 / −60 | R-4.3 | P1 |
| 4 | R-4.5 | 合规风控、错误处理与类型系统 | ~10 | +2,000 / −80 | R-4.1 | P1 |
| 4 | R-4.6 | 策略模板 (taiji-strategy-template) | ~3 | +500 / −10 | R-4.5 | P2 |
| 5 | R-5.1 | LLM 对接层 (taiji-llm) | ~9 | +2,000 / −50 | R-4.3 | P1 |
| 5 | R-5.2 | 回测系统 (taiji-backtest) | ~8 | +1,500 / −40 | R-5.1 | P1 |
| 5 | R-5.3 | 执行交易系统 (taiji-executor) | ~7 | +1,200 / −30 | R-4.5 | P1 |
| 5 | R-5.4 | 风控告警系统 (taiji-alert) | ~5 | +800 / −20 | R-4.5 | P1 |
| 6 | R-6.1 | taiji-abnormal 异常检测 | ~9 | +1,500 / −30 | R-4.5 | P2 |
| 6 | R-6.2 | taiji-agents 与 taiji-blog-gen | ~12 | +2,000 / −50 | — | P2 |
| 6 | R-6.3 | taiji-content 内容引擎 | ~12 | +2,500 / −60 | — | P2 |
| 6 | R-6.4 | taiji-engine-py Python 绑定 | ~8 | +1,200 / −30 | R-4.x | P2 |
| 6 | R-6.5 | taiji-growth 与 taiji-knowledge-graph | ~15 | +2,000 / −50 | — | P2 |
| 6 | R-6.6 | taiji-orderflow 与 taiji-pattern | ~10 | +1,500 / −40 | — | P2 |
| 6 | R-6.7 | taiji-publisher 与 taiji-realtime | ~13 | +2,000 / −50 | — | P2 |
| 6 | R-6.8 | taiji-sentiment、taiji-strategen、taiji-example | ~14 | +2,000 / −50 | — | P2 |
| 7 | R-7.1 | taiji-cli 工具链 | ~8 | +1,500 / −80 | — | P1 |
| 7 | R-7.2 | CLI 应用程序集成 (D) | ~10 | +1,200 / −100 | R-7.1 | P1 |
| 7 | R-7.3 | Skills 全局可用性控制 (E) | ~5 | +600 / −50 | — | P1 |
| 7 | R-7.4 | ACP 协议与 MCP 支持 | ~6 | +800 / −40 | R-7.1 | P2 |
| 8 | R-8.1 | 品牌替换 (G1) | ~10 | +500 / −300 | — | P2 |
| 8 | R-8.2 | 开源治理文档 (G2) | ~8 | +800 / −200 | — | P2 |
| 8 | R-8.3 | 知识库 - Crate 文档 (H1) | ~55 | +6,000 / −100 | 各对应 R | P2 |
| 8 | R-8.4 | 迭代计划与技术文档 (H2) | ~17 | +3,000 / −50 | — | P2 |
| 8 | R-8.5 | 法律、合规与贡献文档 | ~5 | +1,000 / −200 | — | P2 |
| 8 | R-8.6 | Warden/RBAC-Poke 死代码标记 (C) | ~8 | +100 / −2,200 | — | P3 |
| 8 | R-8.7 | Assembly Core Agentic 模块 (后端) | ~25 | +5,000 / −300 | R-2.x | P1 |
| 8 | R-8.8 | Agent Runtime 与执行层 | ~10 | +2,000 / −150 | R-8.7 | P1 |
| 8 | R-8.9 | Contracts 稳定契约 | ~6 | +1,200 / −80 | R-8.7 | P1 |
| 8 | R-8.10 | Services 服务层 (剩余) | ~5 | +800 / −50 | R-8.9 | P1 |
| 8 | R-8.11 | 前端导航与 Session 管理 UI | ~15 | +2,000 / −150 | — | P1 |
| 8 | R-8.12 | 前端 Flow Chat 组件 | ~15 | +2,000 / −150 | — | P2 |
| 8 | R-8.13 | 前端配置与国际化资源 | ~15 | +1,500 / −100 | — | P2 |
| 8 | R-8.14 | 桌面集成与 Tauri API | ~10 | +800 / −100 | — | P1 |
| 8 | R-8.15 | 杂项文件收尾 | ~40 | +3,000 / −300 | — | P3 |
| | **合计** | | **~499** | **+69,756 / −3,446** | | |

---

## 详细

### Wave 0: 基础基座

构建基础设施、工作空间配置与产品定义。所有后续 PR 依赖此波次正确合并。

---

#### R-0.1: 工作空间与构建配置

- **文件清单:**
  - `Cargo.toml`（workspace root，成员变更）
  - `package.json`（脚本与依赖变更）
  - `pnpm-workspace.yaml`
  - `pnpm-lock.yaml`
  - `.gitignore`
  - `.github/CODEOWNERS`
  - `release-please-config.json`
  - `scripts/cargo-target-gc.mjs`
  - `scripts/dev.cjs`
  - `scripts/embed-server.py`
  - `scripts/gen-plan.py`
  - `scripts/sync-upstream.ps1`
  - `scripts/test-onnx.py`
- **文件数:** ~13
- **新增行数:** ~500
- **删除行数:** ~30
- **功能描述:** 建立 taiji-quant 分支所需的 root-level 构建配置、工作空间成员注册、CI/CD 配置及开发/运维脚本。
- **验收标准:**
  1. `cargo check --workspace` 通过
  2. `pnpm install` 无报错
  3. 所有 workspace member 被正确注册
- **依赖:** 无
- **优先级:** P2

---

#### R-0.2: 量化 crate 基座与产品定义

- **文件清单:**
  - `src/crates/taiji/product.free.toml`
  - `src/crates/taiji/product.standard.toml`
  - `src/crates/taiji/product.toml`
  - `src/crates/taiji/product.ultimate.toml`
  - `src/crates/taiji/LOGGING.md`
  - `src/crates/taiji/THIRD_PARTY_NOTICES.md`
  - `BitFun-Installer/src-tauri/Cargo.toml`（依赖追加）
  - `BitFun-Installer/src-tauri/tauri.conf.json`（配置变更）
  - `BitFun-Installer/src/taiji-icon.png`
  - `CHANGELOG.md`
- **文件数:** ~10
- **新增行数:** ~800
- **删除行数:** ~20
- **功能描述:** 定义 taiji 量化产品的品牌配置（Free/Standard/Ultimate 分层）、日志规范、第三方声明及安装器配置更新。
- **验收标准:**
  1. 产品定义文件通过 `pnpm run product:check`
  2. 安装器构建无报错
  3. 日志规范与主仓库准则一致
- **依赖:** R-0.1
- **优先级:** P2

---

### Wave 1: P0 / P1 核心修复

必须优先合并的紧急缺陷修复。涵盖已知 P0 Bug 及部分 P1 高影响修复。

---

#### R-1.1: L3 128K Token 限制修复

- **文件清单:**
  - `src/crates/assembly/core/src/agentic/...`（`agentic_api.rs` 第 1509 行 `unwrap_or(128128)` → `unwrap_or(1_048_576)`）
  - `src/crates/execution/agent-runtime/src/...`（subagent 路径已修复，主 session 路径需同步）
  - 前端默认值对应文件（`flow-chat.ts` 等）
- **文件数:** ~3
- **新增行数:** ~50
- **删除行数:** ~5
- **功能描述:** 修复主 session 路径下的 L3 128K token 限制。当前 `agentic_api.rs` 第 1509 行 `unwrap_or(128128)` 应改为 `1_048_576`，与 subagent 路径的修复保持一致。同时同步前端默认值。
- **已知 Bug #2 / #3:**
  - Bug #2: `agentic_api.rs` 第 1509 行 `unwrap_or(128128)` 未改为 `1_048_576`（仅 subagent 路径修了）
  - Bug #3: 前端默认值未同步
- **验收标准:**
  1. 主 session 路径下的 token 上限为 `1_048_576`
  2. 前端默认值与后端一致
  3. 集成测试覆盖主 session 和 subagent 两条路径
- **依赖:** 无
- **优先级:** P0

---

#### R-1.2: Cancelled 状态映射修复

- **文件清单:**
  - `src/crates/contracts/core-types/src/lib.rs`（`SubagentResultStatus` 添加 `Cancelled` 变体）
  - `src/crates/assembly/core/src/agentic/...`（取消路径映射修正）
- **文件数:** ~2
- **新增行数:** ~30
- **删除行数:** ~5
- **功能描述:** `SubagentResultStatus` 枚举缺少 `Cancelled` 变体，导致被取消的子 agent 错误映射为 `Failed`。需添加 `Cancelled` 变体并修正取消路径的状态映射。
- **已知 Bug #1:**
  - `SubagentResultStatus` 无 `Cancelled` 变体，取消的子 agent 映射为 `Failed`
- **验收标准:**
  1. `SubagentResultStatus` 包含 `Cancelled` 变体
  2. 取消的子 agent 状态为 `Cancelled` 而非 `Failed`
  3. 现有 `Failed` 路径不受影响
- **依赖:** 无
- **优先级:** P0

---

#### R-1.3: Windows 兼容性修复 (F1)

- **文件清单:**
  - `src/apps/desktop/src/api/browser_api.rs`
  - `src/apps/desktop/icons/taiji-*.png`（8 个 icon 文件）
  - `src/apps/cli/src/agent/runtime_client.rs`
  - `src/apps/cli/src/daemon/service.rs`
  - `src/apps/cli/src/self_update.rs`
  - `scripts/install-cli.mjs` / `scripts/install-cli.test.mjs`
- **文件数:** ~8
- **新增行数:** ~400
- **删除行数:** ~50
- **功能描述:** 修复 Windows 平台兼容性问题，包括桌面浏览器 API、CLI daemon 服务与安装脚本的 Windows 适配。
- **验收标准:**
  1. Windows 上 `pnpm run desktop:dev` 可正常启动
  2. CLI daemon 服务在 Windows 上正常注册与运行
  3. 安装脚本在 Windows PowerShell 中无错误
- **依赖:** 无
- **优先级:** P0

---

#### R-1.4: 前端 Bug 修复集 (F2)

- **文件清单:**
  - `src/web-ui/src/flow_chat/store/FlowChatStore.ts`
  - `src/web-ui/src/infrastructure/config/components/ModelSelectPresentation.tsx`
  - `src/web-ui/src/infrastructure/config/components/subscriptionLoginCoordinator.ts`
  - `src/web-ui/src/infrastructure/config/components/form-controls/*`（ConfigActions, ConfigCheckbox, ConfigForm, ConfigInput, ConfigSection, ConfigSelect, ConfigStatus, ConfigTextarea）
  - `src/web-ui/src/infrastructure/api/service-api/AgentAPI.ts`
  - `src/web-ui/src/flow_chat/components/ChatInputWorkspaceStrip.tsx`
  - `src/web-ui/src/flow_chat/utils/sessionMetadata.ts`
  - `src/web-ui/src/app/components/NavPanel/sections/sessions/SessionsSection.tsx`
  - `src/web-ui/src/app/startup/startupPerformanceContract.test.ts`
- **文件数:** ~12
- **新增行数:** ~800
- **删除行数:** ~100
- **功能描述:** 修复前端多处 Bug：Flow Chat store、ModelSelect 展示、登录协调器、表单控件、AgentAPI、Workspace Strip、Session 元数据、导航面板 Session 节。
- **验收标准:**
  1. `pnpm run type-check:web` 通过
  2. 各修复点对应测试通过
- **依赖:** 无
- **优先级:** P0

---

#### R-1.5: Task Spawn 与 Workspace 路径修复

- **文件清单:**
  - `src/crates/assembly/core/src/agentic/tools/implementations/task/execution.rs`
  - `src/crates/assembly/core/src/agentic/tools/implementations/task/input.rs`
  - `src/crates/assembly/core/src/agentic/tools/implementations/task/mod.rs`
  - `src/crates/assembly/core/src/agentic/tools/implementations/task/schema.rs`
  - `src/crates/assembly/core/src/agentic/tools/implementations/task/tests.rs`
- **文件数:** ~5
- **新增行数:** ~80
- **删除行数:** ~15
- **功能描述:** 修复 Task spawn 路径中静默吞错的问题（`let _ =` → `if let Err(e) = ... log::warn!`），以及 workspace 路径不一致问题（子节点用 `display_workspace`，父节点用 `project_workspace`）。
- **已知 Bug #4 / #5:**
  - Bug #4: Task spawn 路径 `let _ =` 静默吞错
  - Bug #5: D6 workspace 路径不一致
- **验收标准:**
  1. Task spawn 错误被正确记录日志
  2. workspace 路径一致
- **依赖:** 无
- **优先级:** P1

---

### Wave 2: Session 控制面后端

R-003/R-004 会话的 Session 事件、树形拓扑、级联删除、IPC 协议增强。涉及对上游既有模块的深层修改。

---

#### R-2.1: R-003 Session 事件与上下文修复 (B1)

- **文件清单:**
  - `src/crates/assembly/core/src/agentic/coordination/coordination_store.rs`
  - `src/crates/assembly/core/src/agentic/coordination/coordinator.rs`
  - `src/crates/assembly/core/src/agentic/coordination/mod.rs`
  - `src/crates/assembly/core/src/agentic/coordination/review_propagation.rs`
  - `src/crates/assembly/core/src/agentic/coordination/scheduler.rs`
  - `src/crates/assembly/core/src/agentic/coordination/background_outcomes.rs`
  - `src/crates/assembly/core/src/agentic/events/types.rs`
  - `src/crates/assembly/core/src/agentic/session/session_manager.rs`
  - `src/crates/assembly/core/src/service_agent_runtime.rs`
  - `src/crates/contracts/events/src/agentic.rs`
  - `src/crates/contracts/events/src/frontend_projection.rs`
  - `src/crates/contracts/runtime-ports/src/lib.rs`
- **文件数:** ~12
- **新增行数:** ~2,000
- **删除行数:** ~120
- **功能描述:** 实现 R-003 设计中的 Session 事件传递、协调器状态管理与上下文修复。包括 CoordinationStore 重构、Coordinator 增强、ReviewPropagation 新模块、事件类型扩展。
- **验收标准:** `cargo test -p bitfun-product-capabilities` 通过
- **依赖:** 无
- **优先级:** P0

---

#### R-2.2: R-004 树形拓扑与级联删除 (B2)

- **文件清单:**
  - `src/crates/services/services-core/src/session/tree.rs`（新文件）
  - `src/crates/services/services-core/src/session/lineage.rs`
  - `src/crates/services/services-core/src/session/metadata.rs`
  - `src/crates/services/services-core/src/session/metadata_store.rs`
  - `src/crates/services/services-core/src/session/mod.rs`
  - `src/crates/services/services-core/src/session/types.rs`
  - `src/crates/contracts/core-types/src/session_tree.rs`
  - `src/crates/contracts/core-types/src/session.rs`
  - `src/crates/contracts/core-types/src/lib.rs`
  - `src/crates/execution/agent-runtime/src/session.rs`
  - `src/crates/execution/agent-runtime/src/session_control.rs`
  - `src/crates/execution/agent-runtime/src/runtime.rs`
  - `src/crates/execution/agent-runtime/src/sdk.rs`
  - `src/crates/services/services-core/Cargo.toml`
  - `src/crates/execution/agent-runtime/tests/thread_goal_contracts.rs`
- **文件数:** ~15
- **新增行数:** ~2,500
- **删除行数:** ~150
- **功能描述:** 实现 R-004 设计中的树形 Session 拓扑、级联删除与生命周期管理。新增 `session/tree.rs` 和 `core-types/src/session_tree.rs`，修改 Session 元数据存储与 Runtime 控制。
- **验收标准:** `cargo test --workspace` 通过
- **依赖:** R-2.1
- **优先级:** P0

---

#### R-2.3: IPC 协议增强 (B4)

- **文件清单:**
  - `src/crates/interfaces/acp/Cargo.toml`
  - `src/crates/interfaces/acp/src/client/manager.rs`
  - `src/crates/services/services-integrations/src/mcp/protocol/client_info.rs`
  - `src/crates/services/services-integrations/src/mcp/protocol/transport_remote.rs`
  - `src/crates/services/services-integrations/src/function_agents.rs`
  - `src/crates/execution/agent-runtime/Cargo.toml`
  - `src/crates/execution/tool-execution/src/context.rs`
  - `src/crates/execution/tool-contracts/tests/tool_contracts.rs`
- **文件数:** ~8
- **新增行数:** ~1,200
- **删除行数:** ~80
- **功能描述:** 增强 IPC 协议支持，包括 ACP 客户端管理、MCP 协议传输适配、function_agents 集成及 Runtime 扩展。
- **验收标准:** `cargo check -p bitfun-desktop` 通过
- **依赖:** R-2.2
- **优先级:** P2

---

### Wave 3: 前端 Session UI

#### R-3.1: Session 树前端 UI 组件 (B3)

- **文件清单:**
  - `src/web-ui/src/app/components/NavPanel/sections/sessions/SessionsSection.tsx`
  - `src/web-ui/src/app/components/NavPanel/sections/sessions/SessionsSection.scss`
  - `src/web-ui/src/app/layout/BeeColonyMonitor.tsx`
  - `src/web-ui/src/app/scenes/agents/components/CreateLegionPage.tsx`
  - `src/web-ui/src/app/scenes/agents/components/LegionCard.tsx`
  - `src/web-ui/src/app/scenes/agents/data/orchestration-patterns.ts`
  - `src/web-ui/src/flow_chat/store/FlowChatStore.ts`
  - `src/web-ui/src/flow_chat/types/flow-chat.ts`
  - `src/web-ui/src/infrastructure/api/service-api/LegionPresetAPI.ts`
  - `src/web-ui/src/app/startup/startupPerformanceContract.test.ts`
- **文件数:** ~10
- **新增行数:** ~2,000
- **删除行数:** ~100
- **功能描述:** 前端 Session 树 UI 组件、Legion 模式卡片、CreateLegion 页面、BeeColonyMonitor 监控面板、编排模式数据。
- **验收标准:** `pnpm run type-check:web` 通过
- **依赖:** R-2.2
- **优先级:** P0

---

### Wave 4: 量化引擎核心

全新 crate 领域，实现量化交易引擎的核心管线。R-4.x 内部有强依赖关系。

---

#### R-4.1: taiji-bar 与数据源适配层

- **文件清单:**
  - `src/crates/taiji/taiji-bar/Cargo.toml`
  - `src/crates/taiji/taiji-bar/README.md`
  - `src/crates/taiji/taiji-bar/src/lib.rs`
  - `src/crates/taiji/taiji-engine/src/source/adapter.rs`
  - `src/crates/taiji/taiji-engine/src/source/datasource.rs`
  - `src/crates/taiji/taiji-engine/src/source/mgr.rs`
  - `src/crates/taiji/taiji-engine/src/source/mod.rs`
  - `src/crates/taiji/taiji-engine/src/source/replay.rs`
  - `src/crates/taiji/taiji-engine/src/source/validator.rs`
  - `src/crates/taiji/taiji-engine/src/types/bar.rs`
  - `src/crates/taiji/taiji-engine/src/types/mod.rs`
  - `src/crates/taiji/taiji-engine/src/types/tick.rs`
- **文件数:** ~12
- **新增行数:** ~2,500
- **删除行数:** ~50
- **功能描述:** K线数据结构（taiji-bar）与多数据源适配层（adapter/datasource/mgr/replay/validator）。支持文件、实时 WebSocket 与回放三种模式。
- **验收标准:** `cargo test -p taiji-bar` 通过
- **依赖:** 无
- **优先级:** P1

---

#### R-4.2: Pipeline 管线与状态管理

- **文件清单:**
  - `src/crates/taiji/taiji-engine/src/pipeline/mod.rs`
  - `src/crates/taiji/taiji-engine/src/pipeline/bar_gen.rs`
  - `src/crates/taiji/taiji-engine/src/pipeline/reorg.rs`
  - `src/crates/taiji/taiji-engine/src/pipeline/status.rs`
  - `src/crates/taiji/taiji-engine/src/state/mod.rs`
  - `src/crates/taiji/taiji-engine/src/state/snapshot.rs`
  - `src/crates/taiji/taiji-engine/src/store.rs`
  - `src/crates/taiji/taiji-engine/src/config.rs`
  - `src/crates/taiji/taiji-engine/src/safe_json.rs`
  - `src/crates/taiji/taiji-engine/tests/bar_gen_precision.rs`
  - `src/crates/taiji/taiji-engine/tests/pipeline_integration.rs`
  - `src/crates/taiji/taiji-engine/benches/pipeline_bench.rs`
- **文件数:** ~12
- **新增行数:** ~3,000
- **删除行数:** ~80
- **功能描述:** Pipeline 管线引擎（bar 生成、重组织、状态管理）、配置系统、快照存储与精度测试。
- **验收标准:** `cargo test -p taiji-engine` 通过
- **依赖:** R-4.1
- **优先级:** P1

---

#### R-4.3: DAG 引擎、信号融合与权重校准

- **文件清单:**
  - `src/crates/taiji/taiji-engine/src/dag.rs`
  - `src/crates/taiji/taiji-engine/src/fusion.rs`
  - `src/crates/taiji/taiji-engine/src/fusion/weight_calibrator.rs`
  - `src/crates/taiji/taiji-engine/src/signal.rs`
  - `src/crates/taiji/taiji-engine/src/node.rs`
  - `src/crates/taiji/taiji-engine/src/factory.rs`
  - `src/crates/taiji/taiji-engine/src/types/signal.rs`
  - `src/crates/taiji/taiji-engine/src/types/state.rs`
  - `src/crates/taiji/taiji-engine/src/risk.rs`
  - `src/crates/taiji/taiji-engine/src/lib.rs`
  - `src/crates/taiji/taiji-engine/src/error.rs`
  - `src/crates/taiji/taiji-engine/tests/schema_adapter_test.rs`
  - `src/crates/taiji/taiji-engine/tests/full_pipeline_integration.rs`
  - `src/crates/taiji/taiji-engine/tests/e2e_full_trading.rs`
  - `src/crates/taiji/taiji-engine/Cargo.toml`
- **文件数:** ~15
- **新增行数:** ~3,500
- **删除行数:** ~100
- **功能描述:** DAG 信号依赖图引擎、多信号融合（加权/投票）与权重校准器、节点工厂、类型系统、错误处理。
- **验收标准:** `cargo test -p taiji-engine --tests` 通过
- **依赖:** R-4.1
- **优先级:** P1

---

#### R-4.4: 多智能体辩论系统 (Debate)

- **文件清单:**
  - `src/crates/taiji/taiji-engine/src/debate/mod.rs`
  - `src/crates/taiji/taiji-engine/src/debate/agents.rs`
  - `src/crates/taiji/taiji-engine/src/debate/decision.rs`
  - `src/crates/taiji/taiji-engine/src/debate/orchestrator.rs`
  - `src/crates/taiji/taiji-engine/src/debate/record.rs`
  - `src/crates/taiji/taiji-engine/config/debate_roles.yaml`
  - `src/crates/taiji/taiji-engine/src/compliance.rs`
- **文件数:** ~7
- **新增行数:** ~3,000
- **删除行数:** ~60
- **功能描述:** 多智能体辩论协调器、决策引擎、角色配置（YAML）、合规检查。支持多种辩论策略与记录追踪。
- **验收标准:** `cargo test -p taiji-engine -- debate` 通过
- **依赖:** R-4.3
- **优先级:** P1

---

#### R-4.5: 合规风控、错误处理与类型系统

- **文件清单:**
  - `src/crates/taiji/taiji-engine/src/compliance.rs`
  - `src/crates/taiji/taiji-engine/src/error.rs`
  - `src/crates/taiji/taiji-engine/src/risk.rs`
  - `src/crates/taiji/taiji-engine/src/types/bar.rs`
  - `src/crates/taiji/taiji-engine/src/types/mod.rs`
  - `src/crates/taiji/taiji-engine/src/types/signal.rs`
  - `src/crates/taiji/taiji-engine/src/types/state.rs`
  - `src/crates/taiji/taiji-engine/src/lib.rs`
  - `src/crates/taiji/taiji-engine/Cargo.toml`
  - `src/crates/taiji/taiji-engine/README.md`
- **文件数:** ~10
- **新增行数:** ~2,000
- **删除行数:** ~80
- **功能描述:** 合规检查框架、风控模块、统一错误类型、核心类型系统。
- **验收标准:** `cargo check -p taiji-engine` 通过
- **依赖:** R-4.1
- **优先级:** P1

---

#### R-4.6: 策略模板 (taiji-strategy-template)

- **文件清单:**
  - `src/crates/taiji/taiji-strategy-template/Cargo.toml`
  - `src/crates/taiji/taiji-strategy-template/README.md`
  - `src/crates/taiji/taiji-strategy-template/src/lib.rs`
- **文件数:** ~3
- **新增行数:** ~500
- **删除行数:** ~10
- **功能描述:** 策略模板 crate，提供策略开发的标准化接口与示例。
- **验收标准:** `cargo check -p taiji-strategy-template` 通过
- **依赖:** R-4.5
- **优先级:** P2

---

### Wave 5: 量化服务层

基于量化引擎核心（R-4.x）的服务层。各 PR 可并行（均依赖不同 R-4.x）。

---

#### R-5.1: LLM 对接层 (taiji-llm)

- **文件清单:**
  - `src/crates/taiji/taiji-llm/Cargo.toml`
  - `src/crates/taiji/taiji-llm/README.md`
  - `src/crates/taiji/taiji-llm/src/lib.rs`
  - `src/crates/taiji/taiji-llm/src/client.rs`
  - `src/crates/taiji/taiji-llm/src/embedding.rs`
  - `src/crates/taiji/taiji-llm/src/types.rs`
  - `src/crates/taiji/taiji-llm/src/provider/mod.rs`
  - `src/crates/taiji/taiji-llm/src/provider/bitfun.rs`
  - `src/crates/taiji/taiji-llm/src/provider/local.rs`
- **文件数:** ~9
- **新增行数:** ~2,000
- **删除行数:** ~50
- **功能描述:** LLM 对接层，支持 BitFun 与本地（Ollama）两种 provider，含 embedding API。
- **已知 Bug #8:** `chat_stream()` 中 `todo!()` 宏可能导致运行时 panic
- **验收标准:** `cargo check -p taiji-llm` 通过
- **依赖:** R-4.3
- **优先级:** P1

---

#### R-5.2: 回测系统 (taiji-backtest)

- **文件清单:**
  - `src/crates/taiji/taiji-backtest/Cargo.toml`
  - `src/crates/taiji/taiji-backtest/README.md`
  - `src/crates/taiji/taiji-backtest/src/lib.rs`
  - `src/crates/taiji/taiji-backtest/src/config.rs`
  - `src/crates/taiji/taiji-backtest/src/runner.rs`
  - `src/crates/taiji/taiji-backtest/src/stats.rs`
  - `src/crates/taiji/taiji-backtest/src/trade_record.rs`
  - `src/crates/taiji/taiji-backtest/src/walk_forward.rs`
- **文件数:** ~8
- **新增行数:** ~1,500
- **删除行数:** ~40
- **功能描述:** 回测运行器、统计指标、交易记录、Walk-Forward 优化。
- **验收标准:** `cargo test -p taiji-backtest` 通过
- **依赖:** R-5.1
- **优先级:** P1

---

#### R-5.3: 执行交易系统 (taiji-executor)

- **文件清单:**
  - `src/crates/taiji/taiji-executor/Cargo.toml`
  - `src/crates/taiji/taiji-executor/README.md`
  - `src/crates/taiji/taiji-executor/src/lib.rs`
  - `src/crates/taiji/taiji-executor/src/bridge.rs`
  - `src/crates/taiji/taiji-executor/src/order_mgr.rs`
  - `src/crates/taiji/taiji-executor/src/position.rs`
  - `src/crates/taiji/taiji-executor/src/types.rs`
- **文件数:** ~7
- **新增行数:** ~1,200
- **删除行数:** ~30
- **功能描述:** 订单管理、持仓管理、交易执行桥接。
- **验收标准:** `cargo check -p taiji-executor` 通过
- **依赖:** R-4.5
- **优先级:** P1

---

#### R-5.4: 风控告警系统 (taiji-alert)

- **文件清单:**
  - `src/crates/taiji/taiji-alert/Cargo.toml`
  - `src/crates/taiji/taiji-alert/README.md`
  - `src/crates/taiji/taiji-alert/src/lib.rs`
  - `src/crates/taiji/taiji-alert/src/alerters.rs`
  - `src/crates/taiji/taiji-alert/src/heartbeat.rs`
- **文件数:** ~5
- **新增行数:** ~800
- **删除行数:** ~20
- **功能描述:** 多通道风控告警（邮件、钉钉、飞书等）、心跳检测。
- **验收标准:** `cargo check -p taiji-alert` 通过
- **依赖:** R-4.5
- **优先级:** P1

---

### Wave 6: 量化扩展

纯新增 crate，各 PR 可并行。

---

#### R-6.1: taiji-abnormal 异常检测

- **文件清单:**
  - `src/crates/taiji/taiji-abnormal/Cargo.toml`
  - `src/crates/taiji/taiji-abnormal/README.md`
  - `src/crates/taiji/taiji-abnormal/src/lib.rs`
  - `src/crates/taiji/taiji-abnormal/src/corr_fracture.rs`
  - `src/crates/taiji/taiji-abnormal/src/gap_alert.rs`
  - `src/crates/taiji/taiji-abnormal/src/scorecard.rs`
  - `src/crates/taiji/taiji-abnormal/src/trend_accel.rs`
  - `src/crates/taiji/taiji-abnormal/src/vol_anomaly.rs`
  - `src/crates/taiji/taiji-abnormal/src/vol_regime.rs`
- **文件数:** ~9
- **新增行数:** ~1,500
- **删除行数:** ~30
- **功能描述:** 异常检测模块：相关断裂、缺口告警、评分卡、趋势加速、量价异常、波动率体制。
- **验收标准:** `cargo test -p taiji-abnormal` 通过
- **依赖:** R-4.5
- **优先级:** P2

---

#### R-6.2: taiji-agents 与 taiji-blog-gen

- **文件清单:**
  - `src/crates/taiji/taiji-agents/README.md`
  - `src/crates/taiji/taiji-agents/decision-agent.md`
  - `src/crates/taiji/taiji-agents/delta-agent.md`
  - `src/crates/taiji/taiji-agents/magnet-agent.md`
  - `src/crates/taiji/taiji-agents/resonance-agent.md`
  - `src/crates/taiji/taiji-agents/risk-agent.md`
  - `src/crates/taiji/taiji-agents/structure-agent.md`
  - `src/crates/taiji/taiji-agents/thrust-agent.md`
  - `src/crates/taiji/taiji-blog-gen/Cargo.toml`
  - `src/crates/taiji/taiji-blog-gen/README.md`
  - `src/crates/taiji/taiji-blog-gen/src/main.rs`
  - `src/crates/taiji/taiji-blog-gen/templates/*.tera`（3 模板）
  - `src/crates/taiji/taiji-blog-gen/test_data/mock_agent.json`
- **文件数:** ~12
- **新增行数:** ~2,000
- **删除行数:** ~50
- **功能描述:** 量化智能体设计文档（决策、Delta、磁铁、共振、风控、结构、推力）与博客生成器。
- **验收标准:** 文档审核
- **依赖:** 无
- **优先级:** P2

---

#### R-6.3: taiji-content 内容引擎

- **文件清单:**
  - `src/crates/taiji/taiji-content/Cargo.toml`
  - `src/crates/taiji/taiji-content/README.md`
  - `src/crates/taiji/taiji-content/src/lib.rs`
  - `src/crates/taiji/taiji-content/src/annotation.rs`
  - `src/crates/taiji/taiji-content/src/chart_option.rs`
  - `src/crates/taiji/taiji-content/src/composer.rs`
  - `src/crates/taiji/taiji-content/src/cron_job.rs`
  - `src/crates/taiji/taiji-content/src/kline_renderer.rs`
  - `src/crates/taiji/taiji-content/src/live_stream.rs`
  - `src/crates/taiji/taiji-content/src/types/*.rs`（5 文件）
- **文件数:** ~12
- **新增行数:** ~2,500
- **删除行数:** ~60
- **功能描述:** 内容合成引擎：图表渲染、标注、合成器、定时任务、实时流与类型定义。
- **验收标准:** `cargo check -p taiji-content` 通过
- **依赖:** 无
- **优先级:** P2

---

#### R-6.4: taiji-engine-py Python 绑定

- **文件清单:**
  - `src/crates/taiji/taiji-engine-py/Cargo.toml`
  - `src/crates/taiji/taiji-engine-py/README.md`
  - `src/crates/taiji/taiji-engine-py/pyproject.toml`
  - `src/crates/taiji/taiji-engine-py/src/lib.rs`
  - `src/crates/taiji/taiji-engine-py/src/cache.rs`
  - `src/crates/taiji/taiji-engine-py/src/obs_builder.rs`
  - `src/crates/taiji/taiji-engine-py/src/reward_calculator.rs`
  - `src/crates/taiji/taiji-engine-py/src/rl_env.rs`
  - `src/crates/taiji/taiji-engine-py/src/python/*.rs`（3 文件）
- **文件数:** ~8
- **新增行数:** ~1,200
- **删除行数:** ~30
- **功能描述:** Python 绑定层：强化学习环境（RL env）、观察构建器、奖励计算器、PyO3 绑定。
- **验收标准:** `cargo check -p taiji-engine-py` 通过
- **依赖:** R-4.x
- **优先级:** P2

---

#### R-6.5: taiji-growth 与 taiji-knowledge-graph

- **文件清单:**
  - `src/crates/taiji/taiji-growth/Cargo.toml`
  - `src/crates/taiji/taiji-growth/README.md`
  - `src/crates/taiji/taiji-growth/src/lib.rs`
  - `src/crates/taiji/taiji-growth/src/email_dispatcher.rs`
  - `src/crates/taiji/taiji-growth/src/publisher_website.rs`
  - `src/crates/taiji/taiji-growth/src/report_md_gen.rs`
  - `src/crates/taiji/taiji-growth/src/task_dag_exec.rs`
  - `src/crates/taiji/taiji-growth/src/task_dag_types.rs`
  - `src/crates/taiji/taiji-growth/src/types.rs`
  - `src/crates/taiji/taiji-growth/templates/*.tera`（5 模板）
  - `src/crates/taiji/taiji-knowledge-graph/Cargo.toml`
  - `src/crates/taiji/taiji-knowledge-graph/README.md`
  - `src/crates/taiji/taiji-knowledge-graph/build.rs`
  - `src/crates/taiji/taiji-knowledge-graph/src/lib.rs`
  - `src/crates/taiji/taiji-knowledge-graph/src/embedding.rs`
  - `src/crates/taiji/taiji-knowledge-graph/src/types.rs`
- **文件数:** ~15
- **新增行数:** ~2,000
- **删除行数:** ~50
- **功能描述:** 增长引擎（邮件调度、发布网站、Markdown 报告、任务 DAG）与知识图谱（嵌入、构建脚本）。
- **验收标准:** `cargo check -p taiji-growth -p taiji-knowledge-graph` 通过
- **依赖:** 无
- **优先级:** P2

---

#### R-6.6: taiji-orderflow 与 taiji-pattern

- **文件清单:**
  - `src/crates/taiji/taiji-orderflow/Cargo.toml`
  - `src/crates/taiji/taiji-orderflow/README.md`
  - `src/crates/taiji/taiji-orderflow/src/lib.rs`
  - `src/crates/taiji/taiji-orderflow/src/ofi.rs`
  - `src/crates/taiji/taiji-orderflow/src/vpin.rs`
  - `src/crates/taiji/taiji-orderflow/src/welford.rs`
  - `src/crates/taiji/taiji-pattern/Cargo.toml`
  - `src/crates/taiji/taiji-pattern/README.md`
  - `src/crates/taiji/taiji-pattern/src/lib.rs`
  - `src/crates/taiji/taiji-pattern/src/dtw.rs`
  - `src/crates/taiji/taiji-pattern/src/index.rs`
  - `src/crates/taiji/taiji-pattern/src/node.rs`
- **文件数:** ~10
- **新增行数:** ~1,500
- **删除行数:** ~40
- **功能描述:** 订单流分析（OFI、VPIN、Welford）与模式匹配（DTW、索引、节点）。
- **验收标准:** `cargo test -p taiji-orderflow -p taiji-pattern` 通过
- **依赖:** 无
- **优先级:** P2

---

#### R-6.7: taiji-publisher 与 taiji-realtime

- **文件清单:**
  - `src/crates/taiji/taiji-publisher/AGENTS.md`
  - `src/crates/taiji/taiji-publisher/Cargo.toml`
  - `src/crates/taiji/taiji-publisher/README.md`
  - `src/crates/taiji/taiji-publisher/src/lib.rs`
  - `src/crates/taiji/taiji-publisher/src/biliup.rs`
  - `src/crates/taiji/taiji-publisher/src/process_util.rs`
  - `src/crates/taiji/taiji-publisher/src/publish_scheduler.rs`
  - `src/crates/taiji/taiji-publisher/src/publisher_twitter.rs`
  - `src/crates/taiji/taiji-publisher/src/publisher_wechat_mp.rs`
  - `src/crates/taiji/taiji-publisher/src/social_auto.rs`
  - `src/crates/taiji/taiji-realtime/Cargo.toml`
  - `src/crates/taiji/taiji-realtime/README.md`
  - `src/crates/taiji/taiji-realtime/src/lib.rs`
  - `src/crates/taiji/taiji-realtime/src/channel.rs`
  - `src/crates/taiji/taiji-realtime/src/datasource.rs`
  - `src/crates/taiji/taiji-realtime/src/ws_bridge.rs`
- **文件数:** ~13
- **新增行数:** ~2,000
- **删除行数:** ~50
- **功能描述:** 多平台发布器（B站、Twitter、微信公众号）与实时数据源 WebSocket 桥接。
- **验收标准:** `cargo check -p taiji-publisher -p taiji-realtime` 通过
- **依赖:** 无
- **优先级:** P2

---

#### R-6.8: taiji-sentiment、taiji-strategen、taiji-example

- **文件清单:**
  - `src/crates/taiji/taiji-sentiment/Cargo.toml`
  - `src/crates/taiji/taiji-sentiment/README.md`
  - `src/crates/taiji/taiji-sentiment/src/lib.rs`
  - `src/crates/taiji/taiji-sentiment/src/fgi.rs`
  - `src/crates/taiji/taiji-sentiment/src/node.rs`
  - `src/crates/taiji/taiji-sentiment/src/tokenizer.rs`
  - `src/crates/taiji/taiji-sentiment/config/sentiment_dict.json`
  - `src/crates/taiji/taiji-strategen/Cargo.toml`
  - `src/crates/taiji/taiji-strategen/README.md`
  - `src/crates/taiji/taiji-strategen/src/lib.rs`
  - `src/crates/taiji/taiji-strategen/src/analyzer.rs`
  - `src/crates/taiji/taiji-strategen/src/compiler.rs`
  - `src/crates/taiji/taiji-strategen/src/hypothesis.rs`
  - `src/crates/taiji/taiji-strategen/src/pipeline.rs`
  - `src/crates/taiji/taiji-strategen/src/refiner.rs`
  - `src/crates/taiji/taiji-example/Cargo.toml`
  - `src/crates/taiji/taiji-example/README.md`
  - `src/crates/taiji/taiji-example/src/lib.rs`
- **文件数:** ~14
- **新增行数:** ~2,000
- **删除行数:** ~50
- **功能描述:** 情绪分析（贪婪恐惧指数、NLP tokenizer）、策略生成器（分析、编译、假设、管线、优化）与示例 crate。
- **验收标准:** `cargo check -p taiji-sentiment -p taiji-strategen -p taiji-example` 通过
- **依赖:** 无
- **优先级:** P2

---

### Wave 7: CLI & Skills

taiji-cli 工具链及其集成、Skills 全局可用性控制、ACP/MCP 支持。

---

#### R-7.1: taiji-cli 工具链

- **文件清单:**
  - `src/crates/taiji/taiji-cli/Cargo.toml`
  - `src/crates/taiji/taiji-cli/README.md`
  - `src/crates/taiji/taiji-cli/src/main.rs`
  - `src/crates/taiji/taiji-cli/src/config.rs`
  - `src/crates/taiji/taiji-cli/src/auth.rs`
  - `src/crates/taiji/taiji-cli/src/acp.rs`
  - `src/crates/taiji/taiji-cli/src/mcp.rs`
- **文件数:** ~7
- **新增行数:** ~1,500
- **删除行数:** ~80
- **功能描述:** taiji-cli 命令行工具：配置管理、认证、ACP/MCP 协议支持。
- **验收标准:** `cargo check -p taiji-cli` 通过
- **依赖:** 无
- **优先级:** P1

---

#### R-7.2: CLI 应用程序集成 (D)

- **文件清单:**
  - `src/crates/assembly/core/src/function_agents/port_adapters.rs`
  - `src/crates/assembly/core/src/service/session_usage/service.rs`
  - `src/crates/assembly/core/src/service/config/types.rs`
  - `src/apps/cli/src/agent/runtime_client.rs`
  - `src/apps/cli/src/daemon/service.rs`
  - `src/apps/cli/src/peer_host/commands/session.rs`
  - `src/apps/cli/src/self_update.rs`
  - `src/crates/services/services-core/src/session/metadata.rs`
- **文件数:** ~10
- **新增行数:** ~1,200
- **删除行数:** ~100
- **功能描述:** CLI 与上游服务的集成：port adapter、session usage、config 类型、daemon 服务。
- **验收标准:** `cargo check -p bitfun-cli` 通过
- **依赖:** R-7.1
- **优先级:** P1

---

#### R-7.3: Skills 全局可用性控制 (E)

- **文件清单:**
  - `src/crates/assembly/core/src/agentic/agents/mod.rs`
  - `src/crates/assembly/core/src/agentic/agents/registry/builtin.rs`
  - `src/crates/assembly/core/src/agentic/agents/registry/external.rs`
  - `src/crates/assembly/core/src/agentic/agents/registry/mod.rs`
  - `src/crates/assembly/core/src/agentic/goal_mode/mod.rs`
  - `src/crates/assembly/core/src/agentic/memories/runner.rs`
  - `src/crates/assembly/core/src/agentic/memories/startup.rs`
  - `src/crates/assembly/core/src/agentic/persistence/manager.rs`
- **文件数:** ~8
- **新增行数:** ~600
- **删除行数:** ~50
- **功能描述:** Skills 全局可用性控制：agent 注册表（builtin/external）的可用性过滤、goal mode 增强。
- **验收标准:** `cargo test -p bitfun-product-capabilities` 通过
- **依赖:** 无
- **优先级:** P1

---

#### R-7.4: ACP 协议与 MCP 支持

- **文件清单:**
  - `src/crates/taiji/taiji-cli/src/acp.rs`
  - `src/crates/taiji/taiji-cli/src/mcp.rs`
  - `src/crates/interfaces/acp/Cargo.toml`
  - `src/crates/interfaces/acp/src/client/manager.rs`
  - `src/crates/services/services-integrations/src/mcp/protocol/client_info.rs`
  - `src/crates/services/services-integrations/src/mcp/protocol/transport_remote.rs`
- **文件数:** ~6
- **新增行数:** ~800
- **删除行数:** ~40
- **功能描述:** ACP 协议客户端与 MCP 传输适配器的 CLI 侧集成。
- **验收标准:** `cargo check -p taiji-cli -p bitfun-acp` 通过
- **依赖:** R-7.1
- **优先级:** P2

---

### Wave 8: 品牌文档 + 后端核心 + 前端 UI + 死代码 + 杂项

各 PR 无交叉依赖，可并行。

---

#### R-8.1: 品牌替换 (G1)

- **文件清单:**
  - `src/apps/desktop/icons/taiji-*.png`（8 icons）
  - `src/web-ui/public/taiji-icon-128.png`
  - `src/web-ui/public/taiji-icon.png`
  - `BitFun-Installer/src/taiji-icon.png`
  - `src/web-ui/index.html`
  - `src/web-ui/preview.html`
- **文件数:** ~10
- **新增行数:** ~500
- **删除行数:** ~300
- **功能描述:** 品牌替换：桌面图标、Web UI 图标、安装器图标、HTML 标题。
- **验收标准:** 构建后图标正确显示
- **依赖:** 无
- **优先级:** P2

---

#### R-8.2: 开源治理文档 (G2)

- **文件清单:**
  - `ACKNOWLEDGMENTS.md`
  - `CODE_OF_CONDUCT.md`
  - `CHANGELOG.md`
  - `PR-BODY.md`
  - `Git提交规范.md`
  - `Rust工具链配置建议.md`
  - `代码审查标准_Checklist.md`
  - `代码审查流程文档.md`
  - `技术债务治理策略.md`
- **文件数:** ~9
- **新增行数:** ~800
- **删除行数:** ~200
- **功能描述:** 开源治理文档：行为准则、贡献指南、变更日志、PR 模板、代码审查标准、技术债务策略。
- **验收标准:** 文档审核
- **依赖:** 无
- **优先级:** P2

---

#### R-8.3: 知识库 - Crate 文档 (H1)

- **文件清单:**
  - `docs/knowledge-base/index.md`
  - `docs/knowledge-base/dependency-graph.md`
  - `docs/knowledge-base/crates/bitfun/`（22 文件）
  - `docs/knowledge-base/crates/taiji/`（18 文件）
  - `docs/knowledge-base/features/`（5 文件）
  - `docs/knowledge-base/references/`（2 文件）
- **文件数:** ~55
- **新增行数:** ~6,000
- **删除行数:** ~100
- **功能描述:** 知识库文档：bitfun 与 taiji crate 文档、功能说明、引用索引、数据流图。
- **验收标准:** 文档审核
- **依赖:** 各对应 R
- **优先级:** P2

---

#### R-8.4: 迭代计划与技术文档 (H2)

- **文件清单:**
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
  - `docs/code-map-taiji-quant.md`
- **文件数:** ~17
- **新增行数:** ~3,000
- **删除行数:** ~50
- **功能描述:** 迭代计划文档：R-003/R-004 审计/设计/调度/交接文档、Product CLI/MCP/ACP 类型、RBAC Poke 计划。
- **验收标准:** 文档审核
- **依赖:** 无
- **优先级:** P2

---

#### R-8.5: 法律、合规与贡献文档

- **文件清单:**
  - `docs/legal/upstream-authorization.md`
  - `ACKNOWLEDGMENTS.md`
  - `CODE_OF_CONDUCT.md`
  - `docs/taiji-quant-change-summary.md`
  - `.workbuddy/HANDBOOK.md`
- **文件数:** ~5
- **新增行数:** ~1,000
- **删除行数:** ~200
- **功能描述:** 法律授权声明、行为准则、致谢、变更总结。
- **验收标准:** 文档审核
- **依赖:** 无
- **优先级:** P2

---

#### R-8.6: Warden/RBAC-Poke 死代码标记 (C)

- **文件清单:**
  - `src/crates/assembly/core/src/agentic/warden/mod.rs`
  - `src/crates/assembly/core/src/agentic/warden/poisson.rs`
  - `src/crates/assembly/core/src/agentic/warden/punishment_executor.rs`
  - `src/crates/assembly/core/src/agentic/warden/SKILL.md`
  - `src/crates/execution/tool-contracts/src/poke.rs`
  - `src/crates/execution/tool-contracts/src/framework.rs`（部分）
  - `src/crates/assembly/core/src/agentic/tools/restrictions.rs`（部分）
  - `src/crates/assembly/core/tests/rbac_poke_integration.rs`
- **文件数:** ~8
- **新增行数:** ~100（标记注释）
- **删除行数:** ~2,200（死代码移除）
- **功能描述:** 标记或移除 Warden/Poisson/PunishmentExecutor/Poke 体系死代码（~2,152 行，100% 无调用方）。保留 RBAC 查询路径，移除未接入的分配路径。
- **验收标准:** `cargo check --workspace` 通过，无死代码警告
- **依赖:** 无
- **优先级:** P3

---

#### R-8.7: Assembly Core Agentic 模块 (后端)

- **文件清单:**
  - `src/crates/assembly/core/src/agentic/mod.rs`
  - `src/crates/assembly/core/src/agentic/session/session_manager.rs`
  - `src/crates/assembly/core/src/agentic/tools/implementations/agent_wait_tool.rs`
  - `src/crates/assembly/core/src/agentic/tools/implementations/glob_tool.rs`
  - `src/crates/assembly/core/src/agentic/tools/implementations/session_control_tool.rs`
  - `src/crates/assembly/core/src/agentic/tools/implementations/session_history_tool.rs`
  - `src/crates/assembly/core/src/agentic/tools/implementations/session_message_tool.rs`
  - `src/crates/assembly/core/src/agentic/tools/implementations/task/*.rs`
  - `src/crates/assembly/core/src/agentic/tools/mod.rs`
  - `src/crates/assembly/core/src/agentic/tools/pipeline/tool_pipeline.rs`
  - `src/crates/assembly/core/src/agentic/tools/pipeline/types.rs`
  - `src/crates/assembly/core/src/agentic/tools/restrictions.rs`
  - `src/crates/assembly/core/src/agentic/tools/tool_context_runtime.rs`
  - `src/crates/assembly/core/src/service_agent_runtime.rs`
  - `src/crates/assembly/core/Cargo.toml`
- **文件数:** ~25
- **新增行数:** ~5,000
- **删除行数:** ~300
- **功能描述:** Assembly Core 层 Agentic 模块：session manager、tool 实现（agent_wait, glob, session_control, session_history, session_message, task）、pipeline、restrictions、tool context runtime。
- **验收标准:** `cargo test -p bitfun-product-capabilities` 通过
- **依赖:** R-2.x
- **优先级:** P1

---

#### R-8.8: Agent Runtime 与执行层

- **文件清单:**
  - `src/crates/execution/agent-runtime/src/runtime.rs`
  - `src/crates/execution/agent-runtime/src/sdk.rs`
  - `src/crates/execution/agent-runtime/src/session.rs`
  - `src/crates/execution/agent-runtime/src/session_control.rs`
  - `src/crates/execution/agent-runtime/tests/thread_goal_contracts.rs`
  - `src/crates/execution/agent-runtime/Cargo.toml`
  - `src/crates/execution/tool-contracts/src/lib.rs`
  - `src/crates/execution/tool-contracts/src/framework.rs`
  - `src/crates/execution/tool-contracts/src/poke.rs`
  - `src/crates/execution/tool-contracts/tests/tool_contracts.rs`
- **文件数:** ~10
- **新增行数:** ~2,000
- **删除行数:** ~150
- **功能描述:** Agent Runtime 执行层增强：runtime 主循环、SDK 扩展、session 控制、thread_goal 测试。
- **验收标准:** `cargo test -p bitfun-agent-runtime` 通过
- **依赖:** R-8.7
- **优先级:** P1

---

#### R-8.9: Contracts 稳定契约

- **文件清单:**
  - `src/crates/contracts/core-types/src/lib.rs`
  - `src/crates/contracts/core-types/src/session.rs`
  - `src/crates/contracts/core-types/src/session_tree.rs`
  - `src/crates/contracts/events/src/agentic.rs`
  - `src/crates/contracts/events/src/frontend_projection.rs`
  - `src/crates/contracts/runtime-ports/src/lib.rs`
- **文件数:** ~6
- **新增行数:** ~1,200
- **删除行数:** ~80
- **功能描述:** 稳定契约层：core-types（session, session_tree）、events（agentic, frontend_projection）、runtime-ports。
- **验收标准:** `cargo check -p bitfun-core-types -p bitfun-events -p bitfun-runtime-ports` 通过
- **依赖:** R-8.7
- **优先级:** P1

---

#### R-8.10: Services 服务层 (剩余)

- **文件清单:**
  - `src/crates/services/services-core/src/session/mod.rs`
  - `src/crates/services/services-core/src/session/types.rs`
  - `src/crates/services/services-core/src/session/metadata.rs`
  - `src/crates/services/services-core/Cargo.toml`
  - `src/crates/services/services-integrations/src/function_agents.rs`
- **文件数:** ~5
- **新增行数:** ~800
- **删除行数:** ~50
- **功能描述:** Services 层剩余变更：session 模块增强、integration function_agents。
- **验收标准:** `cargo check -p bitfun-services-core -p bitfun-services-integrations` 通过
- **依赖:** R-8.9
- **优先级:** P1

---

#### R-8.11: 前端导航与 Session 管理 UI

- **文件清单:**
  - `src/web-ui/src/app/components/NavPanel/sections/sessions/SessionsSection.tsx`
  - `src/web-ui/src/app/components/NavPanel/sections/sessions/SessionsSection.scss`
  - `src/web-ui/src/app/layout/BeeColonyMonitor.tsx`
  - `src/web-ui/src/app/scenes/agents/components/CreateLegionPage.tsx`
  - `src/web-ui/src/app/scenes/agents/components/LegionCard.tsx`
  - `src/web-ui/src/app/scenes/agents/data/orchestration-patterns.ts`
  - `src/web-ui/src/infrastructure/api/service-api/LegionPresetAPI.ts`
  - `src/web-ui/src/shared/types/session-history.ts`
  - `src/web-ui/src/flow_chat/components/ChatInputWorkspaceStrip.tsx`
  - `src/web-ui/src/flow_chat/components/ChatInputWorkspaceStrip.scss`
  - `src/web-ui/src/flow_chat/hooks/useThreadGoalController.ts`
  - `src/web-ui/src/flow_chat/services/AgenticEventListener.ts`
  - `src/web-ui/src/flow_chat/services/flow-chat-manager/EventHandlerModule.ts`
  - `src/web-ui/src/flow_chat/services/goalService.ts`
  - `src/web-ui/src/flow_chat/components/ChatInput.tsx`
- **文件数:** ~15
- **新增行数:** ~2,000
- **删除行数:** ~150
- **功能描述:** 前端导航与 Session 管理 UI：BeeColonyMonitor、Legion 模式、ChatInputWorkspaceStrip、Session 元数据、事件监听。
- **验收标准:** `pnpm run type-check:web` 通过
- **依赖:** 无
- **优先级:** P1

---

#### R-8.12: 前端 Flow Chat 组件

- **文件清单:**
  - `src/web-ui/src/flow_chat/store/FlowChatStore.ts`
  - `src/web-ui/src/flow_chat/types/flow-chat.ts`
  - `src/web-ui/src/flow_chat/components/modern/ExportImageButton.tsx`
  - `src/web-ui/src/flow_chat/components/modern/ModernFlowChatContainer.tsx`
  - `src/web-ui/src/flow_chat/utils/sessionMetadata.ts`
  - `src/web-ui/src/flow_chat/services/AgenticEventListener.ts`
  - `src/web-ui/src/flow_chat/services/flow-chat-manager/EventHandlerModule.ts`
  - `src/web-ui/src/flow_chat/services/goalService.ts`
  - `src/web-ui/src/flow_chat/hooks/useThreadGoalController.ts`
- **文件数:** ~15
- **新增行数:** ~2,000
- **删除行数:** ~150
- **功能描述:** Flow Chat 组件增强：store 扩展（new_feature_flags, treeView, chainView）、ExportImage、ModernFlowChatContainer。
- **验收标准:** `pnpm run type-check:web` 通过
- **依赖:** 无
- **优先级:** P2

---

#### R-8.13: 前端配置与国际化资源

- **文件清单:**
  - `src/web-ui/src/infrastructure/config/components/ModelSelectPresentation.tsx`
  - `src/web-ui/src/infrastructure/config/components/WorktreesConfig.tsx`
  - `src/web-ui/src/infrastructure/config/components/form-controls/*`（8 文件）
  - `src/web-ui/src/infrastructure/config/components/common/`（5 文件，部分删除）
  - `src/web-ui/src/infrastructure/config/components/index.ts`
  - `src/web-ui/src/infrastructure/api/service-api/AgentAPI.ts`
  - `src/web-ui/src/locales/en-US/flow-chat.json`
  - `src/web-ui/src/locales/zh-CN/flow-chat.json`
  - `src/web-ui/src/locales/zh-TW/flow-chat.json`
- **文件数:** ~15
- **新增行数:** ~1,500
- **删除行数:** ~100
- **功能描述:** 前端配置页面重构：ModelSelect、form-controls 重构、common 组件清理、AgentAPI 扩展、国际化资源同步。
- **验收标准:** `pnpm run type-check:web` 通过
- **依赖:** 无
- **优先级:** P2

---

#### R-8.14: 桌面集成与 Tauri API

- **文件清单:**
  - `src/apps/desktop/src/api/browser_api.rs`
  - `src/apps/desktop/icons/taiji-*.png`（8 icons）
  - `BitFun-Installer/src-tauri/Cargo.toml`
  - `BitFun-Installer/src-tauri/tauri.conf.json`
  - `src/apps/cli/src/agent/runtime_client.rs`
  - `src/apps/cli/src/daemon/service.rs`
  - `src/apps/cli/src/peer_host/commands/session.rs`
  - `src/apps/cli/src/self_update.rs`
- **文件数:** ~10
- **新增行数:** ~800
- **删除行数:** ~100
- **功能描述:** 桌面集成：browser_api 增强、taiji icons、Installer 配置、CLI daemon 服务。
- **验收标准:** `cargo check -p bitfun-desktop` 通过
- **依赖:** 无
- **优先级:** P1

---

#### R-8.15: 杂项文件收尾

- **文件清单:**
  - `.workbuddy/HANDBOOK.md`
  - `.workbuddy/gbrain-start.ps1`
  - `.workbuddy/memory/2026-07-26.md`
  - `.workbuddy/memory/MEMORY.md`
  - `discussion-group/`（30+ 文件）
  - `docs/plans/*.md`（16 文件，未在其他 R-ID 覆盖的）
  - `src/web-ui/src/infrastructure/config/components/ReviewConfig.test.tsx`（删除）
  - `src/web-ui/src/infrastructure/config/components/SkillsConfig.scss`（删除）
  - `src/web-ui/src/infrastructure/config/components/SkillsConfig.tsx`（删除）
  - `src/web-ui/src/infrastructure/config/components/common/`（删除文件）
  - `src/web-ui/src/infrastructure/config/components/subscriptionLoginCoordinator.test.ts`（删除）
- **文件数:** ~40
- **新增行数:** ~3,000
- **删除行数:** ~300
- **功能描述:** 不收归其他 R-ID 的杂项文件：workbuddy 记忆、讨论记录、已删除的前端旧组件。
- **验收标准:** 无测试要求
- **依赖:** 无
- **优先级:** P3

---

## 附录 A：已知 Bug 索引

| ID | 严重度 | 描述 | 归属 R-ID | 状态 |
|:--:|:------:|------|:---------:|:----:|
| 1 | P0 | `SubagentResultStatus` 无 `Cancelled` 变体，取消的子 agent 映射为 `Failed` | R-1.2 | 待修复 |
| 2 | P0 | L3 128K 未修复：`agentic_api.rs` 第 1509 行 `unwrap_or(128128)` 未改为 `1_048_576`（仅 subagent 路径修了） | R-1.1 | 待修复 |
| 3 | P0 | 前端默认值未同步（128K） | R-1.1 | 待修复 |
| 4 | P1 | Task spawn 路径 `let _ =` 静默吞错，应改为 `if let Err(e) = ... log::warn!` | R-1.5 | 待修复 |
| 5 | P1 | D6 workspace 路径不一致：子节点用 `display_workspace`，父节点用 `project_workspace` | R-1.5 | 待修复 |
| 6 | P2 | Goal Chain UI bugs：L0 显示替代 target icon, 多级链空祖先标签, 链未过滤空条目 | R-1.4 | 待修复 |
| 7 | P2 | Subagent 可见性 Bug：前台子 agent 出现在导航面板, 删除后自动恢复 | R-1.4 | 待修复 |
| 8 | P1 | `taiji-llm` `chat_stream()` 中 `todo!()` 宏可能导致运行时 panic | R-5.1 | 待修复 |

## 附录 B：死代码清单

| 模块 | 位置 | 行数 | 归属 R-ID | 说明 |
|------|------|:----:|:---------:|------|
| Warden | `assembly/core/src/agentic/warden/mod.rs` | ~500 | R-8.6 | 100% 无调用方 |
| Poisson 调度 | `assembly/core/src/agentic/warden/poisson.rs` | ~300 | R-8.6 | 100% 无调用方 |
| PunishmentExecutor | `assembly/core/src/agentic/warden/punishment_executor.rs` | ~500 | R-8.6 | 100% 无调用方 |
| Warden SKILL.md | `assembly/core/src/agentic/warden/SKILL.md` | ~200 | R-8.6 | 配套文档，无消费者 |
| Poke tool | `execution/tool-contracts/src/poke.rs` | ~400 | R-8.6 | Poke 体系入口，无调用方 |
| Restrictions 部分 | `execution/tool-contracts/src/restrictions.rs` | ~252 | R-8.6 | 部分代码无调用方 |
| **小计** | | **~2,152** | | |
| RBAC 分配路径 | 分散在各权限模块 | ~500+ | R-8.6 | 查询路径存活，分配路径未接入 UI/API |

## 附录 C：18-PR 建议对照表

| 原编号 | 原始 PR 名称 | 优先级 | 映射 R-ID | 说明 |
|:------:|-------------|:------:|:---------:|------|
| A1 | 量化引擎核心 + 数据层 | P1 | R-4.1, R-4.2, R-4.3 | 拆分为 3 个 PR |
| A2 | 策略与决策层 | P1 | R-4.4, R-4.5, R-4.6 | 拆分为 3 个 PR |
| A3 | 执行与内容发布 | P1 | R-5.1, R-5.2, R-5.3, R-5.4 | 拆分为 4 个 PR |
| A4 | 量化基础设施 | P2 | R-6.1 ~ R-6.8 | 拆分为 8 个 PR |
| B1 | R-003 Session 事件 + 上下文修复 | P0 | R-2.1 | — |
| B2 | R-004 树形拓扑 + 级联删除 | P0 | R-2.2 | — |
| B3 | 前端 Session 树 UI | P0 | R-3.1 | — |
| B4 | IPC 增强 | P2 | R-2.3 | — |
| C | Warden/RBAC-Poke | P3 | R-8.6 | — |
| D | CLI 共享 TUI Runtime | P1 | R-7.1, R-7.2 | 拆分为 2 个 PR |
| E | Skills 全局可用性控制 | P1 | R-7.3 | — |
| F1 | Windows 兼容性修复 | P0 | R-1.3 | — |
| F2 | 前端 Bug 修复 | P0 | R-1.4, R-1.5 | 合并部分 P1 修复 |
| G1 | 品牌替换 | P2 | R-8.1 | — |
| G2 | 开源治理文档 | P2 | R-8.2, R-8.5 | 拆分法律/治理 |
| H1 | 知识库文档 | P2 | R-8.3 | — |
| H2 | 迭代计划文档 | P2 | R-8.4 | — |

---

## 附录 D：建议合并顺序

```
Wave 0 (基础基座)
  R-0.1 (工作空间) → R-0.2 (产品定义)

Wave 1 (P0 核心修复)
  R-1.1 (128K) ── 可并行 ──┐
  R-1.2 (Cancelled) ───────┤
  R-1.3 (Windows 兼容) ────┤  ← 全可并行
  R-1.4 (前端 Bug) ────────┤
  R-1.5 (Task spawn) ──────┘

Wave 2 (Session 控制面后端)
  R-2.1 (R-003 事件) → R-2.2 (R-004 树) → R-2.3 (IPC 增强)

Wave 3 (前端 Session UI)
  R-3.1 (Session 树 UI) ← 依赖 R-2.2

Wave 4 (量化引擎核心)
  R-4.1 (Bar/数据源) → R-4.2 (Pipeline) → R-4.3 (DAG/融合)
  R-4.3 → R-4.4 (Debate)
  R-4.1 → R-4.5 (合规类型) → R-4.6 (策略模板)
  ※ R-4.2 / R-4.5 可并行（均依赖 R-4.1）

Wave 5 (量化服务层)
  均依赖 R-4.x，各 PR 可并行：
  R-5.1 (LLM) → R-5.2 (回测)
  R-5.3 (执行) / R-5.4 (风控) ← 依赖 R-4.5

Wave 6 (量化扩展)
  各 PR 可并行：R-6.1 ~ R-6.8

Wave 7 (CLI & Skills)
  R-7.1 (taiji-cli) → R-7.2 (CLI 集成)
  R-7.1 → R-7.4 (ACP/MCP)
  R-7.3 (Skills) ← 独立

Wave 8 (品牌文档 + 后端核心 + 前端 UI + 死代码 + 杂项)
  各 PR 可并行（无交叉依赖）：
  R-8.1 (品牌) / R-8.2 (治理) / R-8.3 (知识库) / R-8.4 (计划)
  R-8.5 (法律) / R-8.6 (死代码)
  R-8.7 (Assembly) → R-8.8 (Runtime) → R-8.9 (Contracts) → R-8.10 (Services)
  R-8.11 (导航UI) / R-8.12 (Flow Chat) / R-8.13 (配置) / R-8.14 (桌面) / R-8.15 (杂项)
```

---

*本文档基于 git diff 统计（499 变更文件，+69,756 / −3,446 行）、功能分类映射、已知 Bug 审计报告与 super-pr 文档生成。各 PR 的"文件清单"为指引性列表，实际合并时需以 git diff 确认。*
