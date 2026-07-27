# R-ID 矩阵：功能 PR 拆分规划

> 生成日期：2026-07-28  
> 数据来源：r001-diff-files.txt（499 变更文件）、r002-reference-index.md（参考材料索引）、r003-gbrain-insights.md（已知错误列表）、_recon_category_map.md（功能分类映射）  
> 总文件数：499 | 总新增行数：+69,756 | 总删除行数：-3,446

---

## 概览

| 波次 | R-ID | PR 名称 | 文件数 | 预估行数 | 依赖 | 优先级 |
|------|------|---------|:------:|:--------:|:----:|:------:|
| 0 | R-0.1 | 工作区构建配置 | 8 | ~200 | 无 | P0 |
| 0 | R-0.2 | 产品定义配置 | 4 | ~100 | 无 | P0 |
| 0 | R-0.3 | 上游配置组件恢复 + UTF-8 修复 | 12 | ~300 | 无 | P0 |
| 1 | R-1.1 | Windows 兼容性修复 | 2 | ~120 | 无 | P0 |
| 1 | R-1.2 | Known P0 Bug 修复（Cancelled 映射 + L3 128K） | 4 | ~80 | R-0.1 | P0 |
| 1 | R-1.3 | 前端 128K 默认值同步 | 3 | ~30 | R-1.2 | P0 |
| 1 | R-1.4 | 前端 Bug 修复（composer card + mouse glow） | 3 | ~60 | 无 | P0 |
| 2 | R-2.1 | R-003 核心：事件 + dialog turn 注入 + 上下文修复 | 18 | ~1,200 | R-1.2 | P0 |
| 2 | R-2.2 | R-004 核心：depth 继承 + 树形拓扑 + 级联删除 | 25 | ~1,800 | R-2.1 | P0 |
| 2 | R-2.3 | 遗留缺口修复（G-03 let _ = 吞错, G-04 workspace 路径） | 3 | ~50 | R-2.2 | P1 |
| 3 | R-3.1 | 前端 Session 树 UI（SessionsSection + FlowChatStore） | 15 | ~800 | R-2.2 | P0 |
| 3 | R-3.2 | Goal Chain Breadcrumb UI 修复 | 3 | ~100 | R-3.1 | P1 |
| 3 | R-3.3 | Subagent 可见性修复 | 3 | ~150 | R-3.1 | P1 |
| 3 | R-3.4 | IPC session 租约 + 共享控制器 | 8 | ~600 | R-2.2 | P2 |
| 4 | R-4.1 | 量化核心引擎 + K 线聚合（taiji-engine + taiji-bar） | 43 | ~9,200 | R-0.1 | P1 |
| 4 | R-4.2 | 计算指标层（abnormal + pattern + orderflow + sentiment） | 20 | ~3,500 | R-4.1 | P1 |
| 4 | R-4.3 | 策略与决策层（taiji-llm + strategen + template + example） | 16 | ~2,800 | R-4.1 | P1 |
| 5 | R-5.1 | 回测与执行（backtest + executor + realtime） | 20 | ~3,000 | R-4.1 | P1 |
| 5 | R-5.2 | 内容与发布（content + publisher + growth + blog-gen） | 35 | ~5,500 | R-4.1 | P1 |
| 5 | R-5.3 | 监控与知识图谱（alert + knowledge-graph） | 8 | ~1,000 | R-4.1 | P1 |
| 5 | R-5.4 | 量化基础设施（engine-py + cli + agents + product configs） | 25 | ~3,500 | R-4.1 | P2 |
| 6 | R-6.1 | CLI 共享 TUI Runtime | 20 | ~2,500 | R-0.1 | P1 |
| 6 | R-6.2 | Skills 全局可用性控制 | 10 | ~600 | R-2.1 | P1 |
| 7 | R-7.1 | 品牌替换 + 图标 | 15 | ~200 | 无 | P2 |
| 7 | R-7.2 | 开源治理文档 | 8 | ~500 | 无 | P2 |
| 7 | R-7.3 | 知识库 crate 分析文档 | 54 | ~3,000 | 无 | P2 |
| 7 | R-7.4 | 迭代计划文档（R-003/R-004/RBAC/CLI-MCP-ACP） | 16 | ~9,300 | 无 | P2 |
| 7 | R-7.5 | 代码地图 + 变更汇总 | 2 | ~600 | 无 | P2 |
| 8 | R-8.1 | DEAD CODE：Warden/RBAC-Poke 标记 + 文档 | 7 | ~2,152 | 无 | P3 |
| 8 | R-8.2 | 修复：taiji-llm chat_stream() todo!() panic | 1 | ~10 | R-4.3 | P1 |
| 8 | R-8.3 | 运维脚本（relay deploy + dev.cjs + sync-upstream 审计） | 6 | ~300 | 无 | P2 |
| 8 | R-8.4 | 讨论组归档（0x9 方案讨论记录） | 30 | ~2,500 | 无 | P3 |
| | **合计** | | **499** | **~69,756 / -3,446** | | |

---

## 详细

### Wave 0: 基础基座

> 说明：这些 PR 是后续所有变更的编译和配置基础。必须先合入。

---

#### R-0.1: 工作区构建配置

- **文件清单**：
  - `Cargo.toml`（workspace root）
  - `package.json`
  - `pnpm-workspace.yaml`
  - `pnpm-lock.yaml`
  - `.gitignore`
  - `scripts/dev.cjs`（消除 DEP0190 DeprecationWarning）
  - `scripts/cargo-target-gc.mjs`
  - `BitFun-Installer/src-tauri/Cargo.toml`
  - `BitFun-Installer/src-tauri/tauri.conf.json`
- **文件数**: ~8
- **新增行数**: ~180
- **删除行数**: ~20
- **功能描述**：调整 workspace 根配置以支持 taiji-quant crate 树。在 `Cargo.toml` 的 `members` 中添加 `src/crates/taiji/*`，在 `pnpm-workspace.yaml` 中添加前端包。同时修复 `dev.cjs` 中的 DEP0190 DeprecationWarning。
- **验收标准**：
  1. `cargo check --workspace` 编译通过
  2. `pnpm install` 无错误
  3. `pnpm run dev:web` 正常启动
- **依赖**: 无
- **优先级**: P0

---

#### R-0.2: 产品定义配置

- **文件清单**：
  - `src/crates/taiji/product.toml`
  - `src/crates/taiji/product.free.toml`
  - `src/crates/taiji/product.standard.toml`
  - `src/crates/taiji/product.ultimate.toml`
- **文件数**: 4
- **新增行数**: ~100
- **删除行数**: 0
- **功能描述**：添加 taiji 量化引擎的产品定义配置。定义 free/standard/ultimate 三级产品能力矩阵（哪些 quant crate 在哪个 tier 可用）。
- **验收标准**：
  1. `pnpm run product:check` 通过
  2. 各 tier 包含正确的 crate 列表
- **依赖**: R-0.1
- **优先级**: P0

---

#### R-0.3: 上游配置组件恢复 + UTF-8 修复

- **文件清单**：
  - `src/web-ui/src/infrastructure/config/components/` 中恢复的通用组件（ConfigPageLayout.tsx, ConfigCollectionItem.tsx 等）
  - `src/web-ui/src/infrastructure/config/components/common/index.ts`
  - `src/web-ui/src/infrastructure/config/components/form-controls/`（ConfigActions, ConfigCheckbox, ConfigForm, ConfigInput, ConfigSection, ConfigSelect, ConfigStatus, ConfigTextarea, index.ts）
  - `src/web-ui/src/infrastructure/config/components/index.ts`
  - `src/web-ui/src/infrastructure/config/components/WorktreesConfig.tsx`
  - `src/web-ui/src/infrastructure/config/components/subscriptionLoginCoordinator.ts`
  - `src/web-ui/src/infrastructure/config/components/ModelSelectPresentation.tsx`
- **文件数**: ~12
- **新增行数**: ~280
- **删除行数**: ~20
- **功能描述**：上游重构破坏了通用配置组件，taiji-quant 分支中的 stub 替换导致了 UTF-8 编码损坏。此 PR 从上游恢复正确的组件实现，修复 UTF-8 编码，并删除空壳 stub。
- **验收标准**：
  1. 配置页面正常渲染（所有 form control 可用）
  2. 无 UTF-8 编码损坏
  3. 删除的组件不阻塞构建
- **依赖**: 无
- **优先级**: P0

---

### Wave 1: 核心修复（P0/P1 Bug）

> 说明：这些 PR 修复已知的最高优先级问题，应在任何功能合入之前处理。

---

#### R-1.1: Windows 兼容性修复

- **文件清单**：
  - `src/crates/services/services-core/src/process_manager.rs`（`CREATE_NO_WINDOW` flag）
  - `src/crates/services/services-core/src/session/memory_workspace.rs`（git CLI → libgit2）
- **文件数**: 2
- **新增行数**: ~100
- **删除行数**: ~20
- **功能描述**：修复 Windows 平台两个兼容性问题：(1) 后台进程 spawn 时传递 `CREATE_NO_WINDOW` 标志，防止弹窗；(2) 使用 libgit2 替代 git CLI 命令执行 workspace 操作，消除子进程开销和安全问题。
- **验收标准**：
  1. Windows 上后台命令不再弹出控制台窗口
  2. workspace 操作使用 libgit2，不依赖系统 git
  3. `cargo check --workspace` 通过
- **依赖**: 无
- **优先级**: P0

---

#### R-1.2: Known P0 Bug 修复（Cancelled 映射 + L3 128K）

- **文件清单**：
  - `src/crates/contracts/events/src/agentic.rs`（SubagentCompletionStatus 增加 Cancelled 变体）
  - `src/crates/assembly/core/src/agentic/coordination/background_outcomes.rs`（Cancelled 映射）
  - `src/crates/execution/agent-runtime/src/session.rs`（已修复 128K→1M，确认完整性）
  - `src/crates/services/services-core/src/session/tree.rs`（L3 兜底路径 `agentic_api.rs:1509` `unwrap_or(128128)` → `1_048_576`）
- **文件数**: 4
- **新增行数**: ~50
- **删除行数**: ~30
- **功能描述**：修复两个 P0 已知 Bug：
  1. **Cancelled 状态映射错误**：`SubagentResultStatus` 枚举缺少 `Cancelled` 变体，导致被取消的子 agent 被错误映射为 `Failed`。需增加 `Cancelled` 变体并更新所有匹配逻辑。
  2. **L3 128K→1M 上下文窗口未修复**：`agentic_api.rs` 中第 1509 行 `unwrap_or(128128)` 仍使用旧的 128K 默认值，未改为 `1_048_576`。主 session 创建路径（agentic/Plan/Cowork）的 L3 兜底也未修复。
- **验收标准**：
  1. 取消的子 agent 状态映射为 `Cancelled` 而非 `Failed`
  2. 所有上下文默认值均使用 1M（1_048_576）
  3. 测试断言同步更新
  4. `cargo test` 通过
- **依赖**: R-0.1
- **优先级**: P0
- **关联已知 Bug**: G-01（Cancelled 映射）, G-02（L3 128K 未修复）

---

#### R-1.3: 前端 128K 默认值同步

- **文件清单**：
  - `src/web-ui/src/flow_chat/types/flow-chat.ts`（前端默认上下文窗口值）
  - `src/web-ui/src/flow_chat/store/FlowChatStore.ts`（默认值使用）
  - `src/web-ui/src/shared/types/session-history.ts`（历史记录类型）
- **文件数**: 3
- **新增行数**: ~20
- **删除行数**: ~10
- **功能描述**：前端默认上下文窗口值从 128K 同步更新为 1M。后端默认值已在 R-1.2 中修复，前端相关常量、类型定义和默认值初始化需要同步。
- **验收标准**：
  1. 前端新建 session 默认上下文窗口为 1M
  2. 前端 UI 显示正确的上下文窗口值
- **依赖**: R-1.2
- **优先级**: P0

---

#### R-1.4: 前端 Bug 修复（composer card + mouse glow）

- **文件清单**：
  - `src/web-ui/src/flow_chat/components/ChatInput.tsx`（composer card 输入换行同步）
  - `src/web-ui/src/flow_chat/components/ChatInput.scss`（相关样式）
  - `src/web-ui/src/flow_chat/components/modern/ExportImageButton.tsx`（mouse glow 避免强制布局）
- **文件数**: 3
- **新增行数**: ~50
- **删除行数**: ~10
- **功能描述**：修复两个前端 Bug：
  1. **Composer card 输入换行同步**（#1802）：输入换行时 composer card 不同步
  2. **Mouse glow 避免强制布局**：使用 `transform` 替代 `top/left` 避免强制回流
- **验收标准**：
  1. 换行输入时 composer card 正确同步
  2. Mouse glow 效果流畅，无 Layout Shift 警告
- **依赖**: 无
- **优先级**: P0

---

### Wave 2: Session 控制面（后端）

> 说明：R-003/R-004 的核心后端改造。这些 PR 依赖 Wave 1 的 P0 Bug 修复。

---

#### R-2.1: R-003 核心（事件 + dialog turn 注入 + 上下文修复）

- **文件清单**：
  - **事件定义**：
    - `src/crates/contracts/events/src/agentic.rs`（新增 `SubagentTurnCompleted` 变体 + `SubagentCompletionStatus` 枚举）
    - `src/crates/contracts/events/src/frontend_projection.rs`（恢复 SubagentTurnCompleted 前端投影）
  - **协调器**：
    - `src/crates/assembly/core/src/agentic/coordination/coordinator.rs`（dialog turn 注入、refresh_session_context_window、审查传播）
    - `src/crates/assembly/core/src/agentic/coordination/mod.rs`
    - `src/crates/assembly/core/src/agentic/coordination/scheduler.rs`
    - `src/crates/assembly/core/src/agentic/coordination/background_outcomes.rs`
    - `src/crates/assembly/core/src/agentic/coordination/coordination_store.rs`
    - `src/crates/assembly/core/src/agentic/coordination/review_propagation.rs`（新增）
  - **Session 管理器**：
    - `src/crates/assembly/core/src/agentic/session/session_manager.rs`
    - `src/crates/assembly/core/src/agentic/persistence/manager.rs`
  - **事件系统**：
    - `src/crates/assembly/core/src/agentic/events/types.rs`
  - **核心类型**：
    - `src/crates/contracts/core-types/src/lib.rs`
    - `src/crates/contracts/core-types/src/session.rs`
  - **Agent 注册**：
    - `src/crates/assembly/core/src/agentic/agents/mod.rs`
    - `src/crates/assembly/core/src/agentic/agents/registry/builtin.rs`
    - `src/crates/assembly/core/src/agentic/agents/registry/external.rs`
    - `src/crates/assembly/core/src/agentic/agents/registry/mod.rs`
  - **Goal mode**：
    - `src/crates/assembly/core/src/agentic/goal_mode/mod.rs`
- **文件数**: 18
- **新增行数**: ~800
- **删除行数**: ~400
- **功能描述**：R-003 核心功能。实现子 agent 完成事件（SubagentTurnCompleted）、自动将子 agent 结果注入父会话 dialog turn、三层上下文窗口修复策略（L1 refresh_session_context_window / L2 SessionConfig 默认值 / L3 agentic_api 兜底）。
- **验收标准**：
  1. 子 agent 完成后，父会话收到 SubagentTurnCompleted 事件
  2. 子 agent 结果自动注入父会话对话流
  3. 上下文窗口默认值从 128K 提升至 1M（三层策略验证）
  4. `cargo test` 通过
- **依赖**: R-1.2（Cancelled 映射修复）
- **优先级**: P0

---

#### R-2.2: R-004 核心（depth 继承 + 树形拓扑 + 级联删除 + 工具互引）

- **文件清单**：
  - **核心类型**：
    - `src/crates/contracts/core-types/src/session_tree.rs`（新增：SessionTreeNode / SessionTreeNodeStatus）
    - `src/crates/contracts/core-types/src/lib.rs`（公开 session_tree 模块）
  - **树管理器**：
    - `src/crates/services/services-core/src/session/tree.rs`（新增：SessionTreeManager，纯内存树结构）
    - `src/crates/services/services-core/src/session/mod.rs`（导出 tree）
    - `src/crates/services/services-core/src/session/types.rs`（depth 字段、should_hide_from_user_lists）
    - `src/crates/services/services-core/src/session/metadata.rs`
    - `src/crates/services/services-core/src/session/metadata_store.rs`（隐藏会话不入索引）
    - `src/crates/services/services-core/src/session/lineage.rs`（谱系持久化）
    - `src/crates/services/services-core/Cargo.toml`
  - **协调器**：
    - `src/crates/assembly/core/src/agentic/coordination/coordinator.rs`（depth 继承、tree.register_child）
    - `src/crates/assembly/core/src/agentic/coordination/background_outcomes.rs`
  - **工具实现**：
    - `src/crates/assembly/core/src/agentic/tools/implementations/session_control_tool.rs`（核心改动：depth 继承、tree 注册、list→树形JSON、cancel 跳过预检、delete 级联）
    - `src/crates/assembly/core/src/agentic/tools/implementations/session_history_tool.rs`（D7 工具互引）
    - `src/crates/assembly/core/src/agentic/tools/implementations/session_message_tool.rs`（D7 工具互引）
    - `src/crates/assembly/core/src/agentic/tools/implementations/task/execution.rs`（前台/后台分叉、depth 传递）
    - `src/crates/assembly/core/src/agentic/tools/implementations/task/mod.rs`（Task list 树形输出）
    - `src/crates/assembly/core/src/agentic/tools/implementations/task/schema.rs`（D7 工具互引）
    - `src/crates/assembly/core/src/agentic/tools/implementations/task/input.rs`
    - `src/crates/assembly/core/src/agentic/tools/implementations/task/tests.rs`
    - `src/crates/assembly/core/src/agentic/tools/implementations/agent_wait_tool.rs`
    - `src/crates/assembly/core/src/agentic/tools/implementations/glob_tool.rs`
    - `src/crates/assembly/core/src/agentic/tools/pipeline/tool_pipeline.rs`
    - `src/crates/assembly/core/src/agentic/tools/pipeline/types.rs`
  - **权限**：
    - `src/crates/assembly/core/src/agentic/tools/restrictions.rs`
    - `src/crates/assembly/core/src/agentic/tools/tool_context_runtime.rs`
  - **运行时**：
    - `src/crates/execution/agent-runtime/src/runtime.rs`
    - `src/crates/execution/agent-runtime/src/sdk.rs`
    - `src/crates/execution/agent-runtime/src/session_control.rs`
    - `src/crates/execution/agent-runtime/Cargo.toml`
  - **工具契约**：
    - `src/crates/execution/tool-contracts/src/framework.rs`
    - `src/crates/execution/tool-contracts/src/lib.rs`
    - `src/crates/execution/tool-execution/src/context.rs`
  - **其他**：
    - `src/crates/assembly/core/src/service_agent_runtime.rs`
    - `src/crates/assembly/core/src/function_agents/port_adapters.rs`
    - `src/crates/assembly/core/src/service/config/types.rs`
    - `src/crates/assembly/core/Cargo.toml`
    - `src/crates/interfaces/acp/src/client/manager.rs`
    - `src/crates/interfaces/acp/Cargo.toml`
    - `src/apps/desktop/src/api/remote_workspace_policy.rs`
- **文件数**: 25
- **新增行数**: ~1,200
- **删除行数**: ~600
- **功能描述**：R-004 核心功能，7 项设计决策（D1-D7）：depth 从 parent 继承 +1、SessionTreeManager 注册、SubagentTurnCompleted 前端投影保留、list→树形 JSON 输出、Cancel 跳过预检查、Delete 级联递归删除、四工具 description 互相引用。
- **验收标准**：
  1. 子 session 自动继承父 session depth +1
  2. SessionTreeManager 正确注册父子关系
  3. `list` 命令返回树形 JSON
  4. `delete` 级联递归删除所有子节点
  5. 四工具（SessionControl/Task/SessionMessage/SessionHistory）description 互引
  6. `cargo test` 通过
- **依赖**: R-2.1
- **优先级**: P0

---

#### R-2.3: 遗留缺口修复（G-03 let _ = 吞错, G-04 workspace 路径）

- **文件清单**：
  - `src/crates/assembly/core/src/agentic/tools/implementations/task/execution.rs`（修复 `let _ =` 静默吞错）
  - `src/crates/services/services-core/src/session/tree.rs`（D6 workspace 路径一致化）
  - `src/crates/assembly/core/src/agentic/coordination/coordinator.rs`（父/子 workspace 路径对齐）
- **文件数**: 3
- **新增行数**: ~30
- **删除行数**: ~20
- **功能描述**：修复 R-004 审计中发现的 2 个遗留缺口：
  1. **G-03 Task spawn 路径 `let _ =` 静默吞错**：在执行 `task.spawn()` 时使用 `let _ =` 丢弃了 `Result`，应改为 `if let Err(e) = ... log::warn!`
  2. **G-04 D6 workspace 路径不一致**：子节点使用 `display_workspace` 而父节点使用 `project_workspace`，应统一
- **验收标准**：
  1. Task spawn 失败时记录 warn 日志而非静默忽略
  2. 父子节点 workspace 路径一致
- **依赖**: R-2.2
- **优先级**: P1

---

### Wave 3: 前端 Session UI

> 说明：前端 UI 层面的 Session 树、Goal Chain 和 Subagent 可见性。依赖 Wave 2 后端 API。

---

#### R-3.1: 前端 Session 树 UI（SessionsSection + FlowChatStore）

- **文件清单**：
  - **树渲染**：
    - `src/web-ui/src/app/components/NavPanel/sections/sessions/SessionsSection.tsx`（会话树渲染、depth 缩进、展开/折叠）
    - `src/web-ui/src/app/components/NavPanel/sections/sessions/SessionsSection.scss`（树形样式）
  - **Store**：
    - `src/web-ui/src/flow_chat/store/FlowChatStore.ts`（addExternalSession depth 存储、级联删除、getSessionTree）
    - `src/web-ui/src/flow_chat/types/flow-chat.ts`（Session 类型增加 depth）
    - `src/web-ui/src/flow_chat/utils/sessionMetadata.ts`（deriveSessionRelationshipFromMetadata depth 传递）
  - **事件监听**：
    - `src/web-ui/src/flow_chat/services/flow-chat-manager/EventHandlerModule.ts`（handleSubagentTurnCompleted）
    - `src/web-ui/src/flow_chat/services/AgenticEventListener.ts`（事件监听绑定）
    - `src/web-ui/src/flow_chat/services/goalService.ts`
    - `src/web-ui/src/flow_chat/hooks/useThreadGoalController.ts`
    - `src/web-ui/src/infrastructure/api/service-api/AgentAPI.ts`（onSubagentTurnCompleted）
  - **UI 组件**：
    - `src/web-ui/src/flow_chat/components/ChatInput.tsx`
    - `src/web-ui/src/flow_chat/components/ChatInputWorkspaceStrip.tsx`
    - `src/web-ui/src/flow_chat/components/ChatInputWorkspaceStrip.scss`
    - `src/web-ui/src/flow_chat/components/modern/ModernFlowChatContainer.tsx`
  - **i18n**：
    - `src/web-ui/src/locales/en-US/flow-chat.json`
    - `src/web-ui/src/locales/zh-CN/flow-chat.json`
    - `src/web-ui/src/locales/zh-TW/flow-chat.json`
- **文件数**: 15
- **新增行数**: ~500
- **删除行数**: ~300
- **功能描述**：前端 Session 树 UI。在 NavPanel 的 SessionsSection 中渲染树形结构（depth 缩进、展开/折叠图标、父子缩进连线）。FlowChatStore 支持树形数据结构的增删改查，前端事件监听绑定后端 SubagentTurnCompleted 事件。
- **验收标准**：
  1. NavPanel SessionsSection 显示树形层级结构
  2. 父子 session 有正确的缩进和连线
  3. 级联删除在前端反映
  4. Subagent 完成后消息出现在父会话流中
  5. `pnpm run type-check:web` 通过
- **依赖**: R-2.2
- **优先级**: P0

---

#### R-3.2: Goal Chain Breadcrumb UI 修复

- **文件清单**：
  - `src/web-ui/src/flow_chat/hooks/useThreadGoalController.ts`
  - `src/web-ui/src/flow_chat/services/goalService.ts`
  - `src/web-ui/src/flow_chat/components/modern/ModernFlowChatContainer.tsx`
- **文件数**: 3
- **新增行数**: ~60
- **删除行数**: ~40
- **功能描述**：修复 R-005 Goal Chain Breadcrumb UI 审计发现的 3 个 Bug：
  1. L0 显示替代 target icon（High）
  2. 多级链空祖先标签（Medium）
  3. 链未过滤空条目（Low）
  根因：`length > 0` 条件过于粗糙。推荐最小修复方案：`.some(e => !!e.goal?.objective)`。
- **验收标准**：
  1. L0 不显示替代图标
  2. 多级链无空标签显示
  3. 链正确过滤空条目
- **依赖**: R-3.1
- **优先级**: P1

---

#### R-3.3: Subagent 可见性修复

- **文件清单**：
  - `src/web-ui/src/app/components/NavPanel/sections/sessions/SessionsSection.tsx`（过滤前台/后台）
  - `src/web-ui/src/flow_chat/store/FlowChatStore.ts`（dismissedSessionIds 跟踪机制）
  - `src/web-ui/src/flow_chat/utils/sessionMetadata.ts`（isBackgroundSubagent 使用）
- **文件数**: 3
- **新增行数**: ~100
- **删除行数**: ~50
- **功能描述**：修复 Subagent 可见性审计发现的 2 个 Bug：
  1. **前台子 agent 出现在导航面板**：`isBackgroundSubagent()` 已定义但未使用，`SessionsSection` 不过滤 `sessionKind`
  2. **删除后自动恢复**：缺少 `dismissedSessionIds` 跟踪机制
- **验收标准**：
  1. 前台子 agent 不在导航面板中显示
  2. 已删除的 session 不会自动恢复
- **依赖**: R-3.1
- **优先级**: P1

---

#### R-3.4: IPC session 租约 + 共享控制器

- **文件清单**：
  - `src/crates/adapters/agent-runtime-ipc/src/client.rs`
  - `src/crates/adapters/agent-runtime-ipc/src/framing.rs`
  - `src/crates/adapters/agent-runtime-ipc/src/handler.rs`（新增）
  - `src/crates/adapters/agent-runtime-ipc/src/ipc.rs`
  - `src/crates/adapters/agent-runtime-ipc/src/lib.rs`
  - `src/crates/adapters/agent-runtime-ipc/src/operation.rs`
  - `src/crates/adapters/agent-runtime-ipc/src/protocol.rs`
  - `src/crates/adapters/agent-runtime-ipc/src/server.rs`
  - `src/crates/adapters/agent-runtime-ipc/src/session_lease.rs`（新增）
  - `src/crates/adapters/agent-runtime-ipc/src/tests/shared_controller.rs`（新增，771 行测试）
  - `src/crates/adapters/agent-runtime-ipc/AGENTS.md`
- **文件数**: 8（含测试）
- **新增行数**: ~500
- **删除行数**: ~100
- **功能描述**：IPC 层增强，新增 session 租约管理（session_lease.rs）和共享控制器（shared_controller.rs 测试）。为多进程场景提供 session 生命周期管理。
- **验收标准**：
  1. IPC session 租约可创建、续期、释放
  2. 共享控制器测试通过
  3. `cargo test` 通过
- **依赖**: R-2.2
- **优先级**: P2

---

### Wave 4: 量化引擎核心

> 说明：taiji-quant 量化交易引擎的核心 crate。这是 taiji-quant 分支的核心负载，约占总增量的 50%。

---

#### R-4.1: 量化核心引擎 + K 线聚合（taiji-engine + taiji-bar）

- **文件清单**：
  - **taiji-engine**（41 文件）：
    - `src/crates/taiji/taiji-engine/Cargo.toml`
    - `src/crates/taiji/taiji-engine/README.md`
    - `src/crates/taiji/taiji-engine/src/lib.rs`
    - `src/crates/taiji/taiji-engine/src/dag.rs`（DAG 拓扑，petgraph + Kahn 排序）
    - `src/crates/taiji/taiji-engine/src/node.rs`（ComputeNode trait，7 个生命周期钩子）
    - `src/crates/taiji/taiji-engine/src/pipeline/mod.rs`
    - `src/crates/taiji/taiji-engine/src/pipeline/bar_gen.rs`
    - `src/crates/taiji/taiji-engine/src/pipeline/reorg.rs`
    - `src/crates/taiji/taiji-engine/src/pipeline/status.rs`
    - `src/crates/taiji/taiji-engine/src/signal.rs`
    - `src/crates/taiji/taiji-engine/src/fusion.rs`
    - `src/crates/taiji/taiji-engine/src/fusion/weight_calibrator.rs`
    - `src/crates/taiji/taiji-engine/src/config.rs`
    - `src/crates/taiji/taiji-engine/src/factory.rs`
    - `src/crates/taiji/taiji-engine/src/store.rs`
    - `src/crates/taiji/taiji-engine/src/error.rs`
    - `src/crates/taiji/taiji-engine/src/compliance.rs`
    - `src/crates/taiji/taiji-engine/src/risk.rs`
    - `src/crates/taiji/taiji-engine/src/safe_json.rs`
    - `src/crates/taiji/taiji-engine/src/types/mod.rs`
    - `src/crates/taiji/taiji-engine/src/types/bar.rs`
    - `src/crates/taiji/taiji-engine/src/types/signal.rs`
    - `src/crates/taiji/taiji-engine/src/types/state.rs`
    - `src/crates/taiji/taiji-engine/src/types/tick.rs`
    - `src/crates/taiji/taiji-engine/src/state/mod.rs`
    - `src/crates/taiji/taiji-engine/src/state/snapshot.rs`
    - `src/crates/taiji/taiji-engine/src/source/mod.rs`
    - `src/crates/taiji/taiji-engine/src/source/adapter.rs`
    - `src/crates/taiji/taiji-engine/src/source/datasource.rs`
    - `src/crates/taiji/taiji-engine/src/source/mgr.rs`
    - `src/crates/taiji/taiji-engine/src/source/replay.rs`
    - `src/crates/taiji/taiji-engine/src/source/validator.rs`
    - `src/crates/taiji/taiji-engine/src/debate/mod.rs`
    - `src/crates/taiji/taiji-engine/src/debate/agents.rs`
    - `src/crates/taiji/taiji-engine/src/debate/decision.rs`
    - `src/crates/taiji/taiji-engine/src/debate/orchestrator.rs`
    - `src/crates/taiji/taiji-engine/src/debate/record.rs`
    - `src/crates/taiji/taiji-engine/config/debate_roles.yaml`
    - `src/crates/taiji/taiji-engine/benches/pipeline_bench.rs`
    - `src/crates/taiji/taiji-engine/tests/bar_gen_precision.rs`
    - `src/crates/taiji/taiji-engine/tests/e2e_full_trading.rs`
    - `src/crates/taiji/taiji-engine/tests/full_pipeline_integration.rs`
    - `src/crates/taiji/taiji-engine/tests/pipeline_integration.rs`
    - `src/crates/taiji/taiji-engine/tests/schema_adapter_test.rs`
  - **taiji-bar**（3 文件）：
    - `src/crates/taiji/taiji-bar/Cargo.toml`
    - `src/crates/taiji/taiji-bar/README.md`
    - `src/crates/taiji/taiji-bar/src/lib.rs`
- **文件数**: 43
- **新增行数**: ~9,000
- **删除行数**: ~200
- **功能描述**：taiji-quant 核心引擎移植。DAG 驱动的 Pipeline 流水线（Tick → BarGenerator → DAG 节点 → Signal 输出），ComputeNode trait 定义 7 个生命周期钩子，多智能体辩论引擎（多头/空头/中立），信号融合+权重校准，数据源适配层（回放/验证/多路由管理）。
- **验收标准**：
  1. `cargo check -p taiji-engine -p taiji-bar` 通过
  2. `cargo test -p taiji-engine -p taiji-bar` 通过（含全量集成测试）
  3. DAG 拓扑排序正确（petgraph + Kahn）
  4. ComputeNode 生命周期钩子按序执行
- **依赖**: R-0.1
- **优先级**: P1

---

#### R-4.2: 计算指标层（taiji-abnormal + taiji-pattern + taiji-orderflow + taiji-sentiment）

- **文件清单**：
  - **taiji-abnormal**（7 文件）：
    - `src/crates/taiji/taiji-abnormal/Cargo.toml`, README.md
    - `src/crates/taiji/taiji-abnormal/src/lib.rs`, corr_fracture.rs, gap_alert.rs, scorecard.rs, trend_accel.rs, vol_anomaly.rs, vol_regime.rs
  - **taiji-pattern**（4 文件）：
    - `src/crates/taiji/taiji-pattern/Cargo.toml`, README.md
    - `src/crates/taiji/taiji-pattern/src/lib.rs`, dtw.rs, index.rs, node.rs
  - **taiji-orderflow**（4 文件）：
    - `src/crates/taiji/taiji-orderflow/Cargo.toml`, README.md
    - `src/crates/taiji/taiji-orderflow/src/lib.rs`, ofi.rs, vpin.rs, welford.rs
  - **taiji-sentiment**（5 文件）：
    - `src/crates/taiji/taiji-sentiment/Cargo.toml`, README.md
    - `src/crates/taiji/taiji-sentiment/src/lib.rs`, fgi.rs, node.rs, tokenizer.rs
    - `src/crates/taiji/taiji-sentiment/config/sentiment_dict.json`
- **文件数**: 20
- **新增行数**: ~3,500
- **删除行数**: ~50
- **功能描述**：计算指标层 crate 移植。异常检测评分卡（相关性断裂、跳空告警、趋势加速、量价异常、波动率体制），图表形态识别（DTW 弹性匹配），订单流分析（VPIN + OFI + Welford 在线统计），市场情绪分析（FGI 恐惧贪婪指数 + 情感词典分词）。
- **验收标准**：
  1. `cargo check` 通过
  2. 各 crate 单元测试通过
  3. abnormal 评分卡输出在合理范围
  4. DTW 匹配正确识别已知形态
- **依赖**: R-4.1
- **优先级**: P1

---

#### R-4.3: 策略与决策层（taiji-llm + taiji-strategen + taiji-strategy-template + taiji-example）

- **文件清单**：
  - **taiji-llm**（7 文件）：
    - `src/crates/taiji/taiji-llm/Cargo.toml`, README.md
    - `src/crates/taiji/taiji-llm/src/lib.rs`, client.rs, embedding.rs, types.rs
    - `src/crates/taiji/taiji-llm/src/provider/bitfun.rs`, local.rs, mod.rs
  - **taiji-strategen**（6 文件）：
    - `src/crates/taiji/taiji-strategen/Cargo.toml`, README.md
    - `src/crates/taiji/taiji-strategen/src/lib.rs`, analyzer.rs, compiler.rs, hypothesis.rs, pipeline.rs, refiner.rs
  - **taiji-strategy-template**（3 文件）：
    - `src/crates/taiji/taiji-strategy-template/Cargo.toml`, README.md, src/lib.rs
  - **taiji-example**（3 文件）：
    - `src/crates/taiji/taiji-example/Cargo.toml`, README.md, src/lib.rs
- **文件数**: 16
- **新增行数**: ~2,800
- **删除行数**: ~50
- **功能描述**：策略与决策层 crate 移植。LLM 客户端抽象层（OpenAI/Claude/BitFun 适配器/本地），LLM 驱动策略自动生成（假设生成→编译→回测→精炼），DualThrust 策略模板，参考 ComputeNode 实现。
- **验收标准**：
  1. `cargo check` 通过
  2. LLM 客户端可连接到配置的 provider
  3. 策略生成管线完整（hypothesis → compile → test → refine）
- **依赖**: R-4.1
- **优先级**: P1

---

### Wave 5: 量化扩展

> 说明：围绕核心引擎的扩展功能，包括回测、执行、内容生成、发布、告警等。

---

#### R-5.1: 回测与执行（taiji-backtest + taiji-executor + taiji-realtime）

- **文件清单**：
  - **taiji-backtest**（6 文件）：Cargo.toml, README.md, src/config.rs, lib.rs, runner.rs, stats.rs, trade_record.rs, walk_forward.rs
  - **taiji-executor**（5 文件）：Cargo.toml, README.md, src/bridge.rs, lib.rs, order_mgr.rs, position.rs, types.rs
  - **taiji-realtime**（5 文件）：Cargo.toml, README.md, src/channel.rs, datasource.rs, lib.rs, ws_bridge.rs
- **文件数**: 20
- **新增行数**: ~3,000
- **删除行数**: ~50
- **功能描述**：回测引擎（walk-forward 交叉验证、统计指标、交易记录），订单执行桥接（订单管理/持仓管理/类型定义），实时行情中枢（WebSocket 桥接/通道管理/数据源适配）。
- **验收标准**：
  1. `cargo check` 通过
  2. 回测引擎 run 完整周期无 panic
  3. Walk-forward 交叉验证结果合理
- **依赖**: R-4.1
- **优先级**: P1

---

#### R-5.2: 内容与发布（taiji-content + taiji-publisher + taiji-growth + taiji-blog-gen）

- **文件清单**：
  - **taiji-content**（12 文件）：Cargo.toml, README.md, src/lib.rs, annotation.rs, chart_option.rs, composer.rs, cron_job.rs, kline_renderer.rs, live_stream.rs, types/bar_types.rs, types/compose_config.rs, types/mod.rs, types/render_config.rs, types/tts_types.rs
  - **taiji-publisher**（8 文件）：AGENTS.md, Cargo.toml, README.md, src/lib.rs, biliup.rs, process_util.rs, publish_scheduler.rs, publisher_twitter.rs, publisher_wechat_mp.rs, social_auto.rs
  - **taiji-growth**（10 文件）：Cargo.toml, README.md, src/lib.rs, email_dispatcher.rs, publisher_website.rs, report_md_gen.rs, task_dag_exec.rs, task_dag_types.rs, types.rs, templates/daily_report.tera, templates/email_confirmation.tera, templates/email_daily_report.tera, templates/email_signal_alert.tera, templates/weekly_report.tera
  - **taiji-blog-gen**（5 文件）：Cargo.toml, README.md, src/main.rs, templates/daily_post.tera, templates/special_topic.tera, templates/weekly_summary.tera, test_data/mock_agent.json
- **文件数**: 35
- **新增行数**: ~5,500
- **删除行数**: ~100
- **功能描述**：内容与发布层 crate 移植。视频渲染管线（K 线渲染/ECharts 合成/TTS/FFmpeg → MP4），多平台发布（微信公众号/Twitter/B 站/自动排期），增长运营（日报/周报生成/邮件分发/DAG 任务编排），博客生成 CLI。
- **验收标准**：
  1. `cargo check` 通过
  2. 内容渲染管线可生成 K 线图表
  3. 发布调度器可排期
- **依赖**: R-4.1
- **优先级**: P1

---

#### R-5.3: 监控与知识图谱（taiji-alert + taiji-knowledge-graph）

- **文件清单**：
  - **taiji-alert**（3 文件）：Cargo.toml, README.md, src/alerters.rs, heartbeat.rs, lib.rs
  - **taiji-knowledge-graph**（5 文件）：Cargo.toml, README.md, build.rs, src/embedding.rs, lib.rs, types.rs
- **文件数**: 8
- **新增行数**: ~1,000
- **删除行数**: ~20
- **功能描述**：多渠道告警系统（钉钉/Slack/邮件/心跳检测），知识图谱构建（嵌入索引/语义搜索）。
- **验收标准**：
  1. `cargo check` 通过
  2. 告警器可发送测试消息
  3. 知识图谱嵌入可正常索引
- **依赖**: R-4.1
- **优先级**: P1

---

#### R-5.4: 量化基础设施（taiji-engine-py + taiji-cli + taiji-agents）

- **文件清单**：
  - **taiji-engine-py**（8 文件）：Cargo.toml, README.md, pyproject.toml, src/cache.rs, lib.rs, obs_builder.rs, reward_calculator.rs, rl_env.rs, src/python/engine_py.rs, python/mod.rs, python/types_py.rs
  - **taiji-cli**（6 文件）：Cargo.toml, README.md, src/acp.rs, auth.rs, config.rs, main.rs, mcp.rs
  - **taiji-agents**（7 文件）：README.md, decision-agent.md, delta-agent.md, magnet-agent.md, resonance-agent.md, risk-agent.md, structure-agent.md, thrust-agent.md
  - **辅助文件**：`src/crates/taiji/LOGGING.md`、`src/crates/taiji/THIRD_PARTY_NOTICES.md`
- **文件数**: 25
- **新增行数**: ~3,500
- **删除行数**: ~50
- **功能描述**：基础设施 crate。Python 绑定（PyO3/RL 训练环境/缓存/Observation Builder/Reward Calculator），独立 CLI 工具（config/auth/MCP/ACP），7 个 AI Agent 提示词模板（structure/delta/magnet/thrust/risk/resonance/decision agent）。
- **验收标准**：
  1. `cargo check` 通过
  2. Python 绑定可编译（pyproject.toml + PyO3）
  3. CLI 工具可执行
- **依赖**: R-4.1
- **优先级**: P2

---

### Wave 6: CLI & Skills

> 说明：独立的功能增强 PR，边界清晰。

---

#### R-6.1: CLI 共享 TUI Runtime

- **文件清单**：
  - `src/apps/cli/src/shared_runtime.rs`（新增，~1,226 行）
  - `src/apps/cli/src/agent/runtime_client.rs`
  - `src/apps/cli/src/main.rs`
  - `src/apps/cli/src/actions.rs`
  - `src/apps/cli/src/chat_state.rs`
  - `src/apps/cli/src/daemon/service.rs`
  - `src/apps/cli/src/modes/chat.rs`
  - `src/apps/cli/src/modes/chat/account.rs`
  - `src/apps/cli/src/modes/chat/commands.rs`
  - `src/apps/cli/src/modes/chat/input.rs`
  - `src/apps/cli/src/modes/chat/run.rs`
  - `src/apps/cli/src/modes/chat/sessions.rs`
  - `src/apps/cli/src/peer_host/commands/session.rs`
  - `src/apps/cli/src/runtime/mod.rs`
  - `src/apps/cli/src/self_update.rs`
  - `src/apps/cli/src/ui/chat/popups.rs`
  - `src/apps/cli/src/ui/session_selector.rs`
  - `src/apps/cli/src/ui/startup.rs`
  - `src/apps/cli/Cargo.toml`
  - `src/apps/cli/tests/exec_cli_contracts.rs`
  - `src/apps/cli/tests/product_assembly_cli.rs`
- **文件数**: 20
- **新增行数**: ~2,200
- **删除行数**: ~300
- **功能描述**：为 CLI 模式引入可选的共享 TUI（终端用户界面）运行时。改善多 session 管理的终端体验。新增 `shared_runtime.rs`（~1,226 行）实现共享运行时核心逻辑。
- **验收标准**：
  1. CLI 共享运行时启动正常
  2. 多 session 管理在 TUI 中可用
  3. `cargo check -p bitfun-cli` 通过
- **依赖**: R-0.1
- **优先级**: P1

---

#### R-6.2: Skills 全局可用性控制

- **文件清单**：
  - **前端**：
    - `src/web-ui/src/app/scenes/skills/SkillsScene.tsx`
    - `src/web-ui/src/app/scenes/skills/SkillsScene.scss`
    - `src/web-ui/src/app/scenes/skills/components/SkillsSuiteView.tsx`
    - `src/web-ui/src/app/scenes/skills/hooks/useInstalledSkills.ts`
    - `src/web-ui/src/app/scenes/skills/hooks/useInstalledSkills.test.tsx`
    - `src/web-ui/src/infrastructure/api/service-api/ConfigAPI.ts`
  - **后端**：
    - `src/crates/execution/agent-runtime/src/skills/mod.rs`
    - `src/crates/execution/agent-runtime/src/skills/selection.rs`
    - `src/crates/execution/agent-runtime/src/skills/types.rs`
    - `src/crates/execution/agent-runtime/tests/skill_contracts.rs`
    - `src/crates/assembly/core/src/agentic/tools/implementations/skills/mode_overrides.rs`
    - `src/crates/assembly/core/src/agentic/tools/implementations/skills/registry.rs`
- **文件数**: 10
- **新增行数**: ~400
- **删除行数**: ~200
- **功能描述**：将 Skills 从会话级配置改造为支持全局启用/禁用。前端增加全局控制 UI，后端 Skills 模块支持全局可用性查询和设置。
- **验收标准**：
  1. 全局 Skills 启用/禁用 UI 可用
  2. 全局配置影响所有会话
  3. `pnpm run type-check:web` 通过
  4. `cargo test` 通过
- **依赖**: R-2.1
- **优先级**: P1

---

### Wave 7: 品牌 & 文档

> 说明：非功能性变更，无代码依赖风险，可随时合入。

---

#### R-7.1: 品牌替换 + 图标

- **文件清单**：
  - `src/apps/desktop/icons/taiji-16x16.png` ~ `taiji-512x512.png`（8 个尺寸）
  - `src/web-ui/public/taiji-icon.png`（391KB）
  - `src/web-ui/public/taiji-icon-128.png`（16KB）
  - `BitFun-Installer/src/taiji-icon.png`
  - `src/web-ui/index.html`（标题/图标引用）
  - `src/web-ui/preview.html`
  - `src/apps/desktop/resources/worker_host.js`
  - `src/apps/desktop/src/api/skill_api.rs`
  - `src/mobile-web/` 相关文件
- **文件数**: 15
- **新增行数**: ~150
- **删除行数**: ~50
- **功能描述**：将 BitFun 品牌替换为 taiji 品牌。替换桌面图标（8 个尺寸）、Web UI 图标、Installer 图标、HTML 标题引用、API 引用等。
- **验收标准**：
  1. 各产品表面显示正确的 taiji 图标
  2. 无残留 BitFun 品牌引用
- **依赖**: 无
- **优先级**: P2

---

#### R-7.2: 开源治理文档

- **文件清单**：
  - `.github/CODEOWNERS`
  - `CODE_OF_CONDUCT.md`
  - `ACKNOWLEDGMENTS.md`
  - `CHANGELOG.md`
  - `SECURITY.md` / `SECURITY_CN.md`
  - `docs/legal/upstream-authorization.md`
  - `PR-BODY.md`
  - `Git提交规范.md`
  - `Rust工具链配置建议.md`
  - `代码审查标准_Checklist.md`
  - `代码审查流程文档.md`
  - `技术债务治理策略.md`
- **文件数**: 8
- **新增行数**: ~500
- **删除行数**: ~20
- **功能描述**：为开源发布准备治理文档。包括 CODEOWNERS、行为准则、致谢名单、变更日志、安全策略、上游授权文档、PR 模板、Git 提交规范、代码审查标准等。
- **验收标准**：
  1. 所有治理文档格式正确
  2. 无敏感信息泄露
- **依赖**: 无
- **优先级**: P2

---

#### R-7.3: 知识库 crate 分析文档

- **文件清单**：
  - `docs/knowledge-base/crates/bitfun/`（30 文件）：bitfun-core, bitfun-agent-runtime, bitfun-agent-stream, bitfun-core-types, bitfun-events, bitfun-runtime-ports, bitfun-services-core, bitfun-services-integrations, bitfun-harness, bitfun-agent-tools, bitfun-ai-adapters, bitfun-claude-code-adapter, bitfun-codex-adapter, bitfun-opencode-adapter, bitfun-product-capabilities, bitfun-product-domains, bitfun-transport, bitfun-webdriver, bitfun-acp, bitfun-external-sources, bitfun-page-function-runtime, bitfun-plugin-runtime-host, bitfun-relay-service, bitfun-runtime-services, bitfun-sdk-host, bitfun-static-hook-support, bitfun-tool-call-jsonrepair, bitfun-tool-packs, terminal-core, tool-runtime
  - `docs/knowledge-base/crates/taiji/`（22 文件）：index.md 及 21 个量化 crate 分析文档
  - `docs/knowledge-base/features/`（5 文件）：quant-engine, agentic-system, ultra-mode, legion-mode, frontend-architecture
  - `docs/knowledge-base/references/`（2 文件）：bug-index.md, data-flows.md
  - `docs/knowledge-base/index.md`
  - `docs/knowledge-base/dependency-graph.md`
- **文件数**: 54
- **新增行数**: ~3,000
- **删除行数**: 0
- **功能描述**：为 taiji-quant 分支建立完整的知识库体系。涵盖上游 BitFun 30 个 crate 分析文档、taiji-quant 21 个量化 crate 分析文档、5 个功能特性文档、Bug 索引和数据流图。
- **验收标准**：
  1. 所有文档格式正确
  2. 索引页链接有效
  3. 依赖关系图与代码一致
- **依赖**: 无
- **优先级**: P2

---

#### R-7.4: 迭代计划文档

- **文件清单**：
  - `docs/plans/r003-requirements.md`
  - `docs/plans/r003-design.md`
  - `docs/plans/r003-dispatch.md`
  - `docs/plans/r003-audit.md`
  - `docs/plans/r003-audit-v1.1.md`
  - `docs/plans/r003-handoff.md`
  - `docs/plans/r004-requirements.md`
  - `docs/plans/r004-design.md`
  - `docs/plans/r004-dispatch.md`
  - `docs/plans/r004-progress-final.md`
  - `docs/plans/phase-rbac-poke-plan.md`
  - `docs/plans/phase-rbac-poke-type-contract.md`
  - `docs/plans/phase-rbac-poke-dispatch-prompts.md`
  - `docs/plans/product-cli-mcp-acp-plan.md`
  - `docs/plans/product-cli-mcp-acp-types.md`
  - `docs/plans/product-cli-mcp-acp-dispatch.md`
- **文件数**: 16
- **新增行数**: ~9,315
- **删除行数**: 0
- **功能描述**：完整的迭代计划文档。包括 R-003（~1,747 行）、R-004（~2,140 行）、RBAC+Poke（~791 行）、CLI+MCP+ACP（~5,038 行）四个计划系列。
- **验收标准**：
  1. 所有计划文档格式正确
  2. 文档间引用关系正确
- **依赖**: 无
- **优先级**: P2

---

#### R-7.5: 代码地图 + 变更汇总

- **文件清单**：
  - `docs/code-map-taiji-quant.md`（556 行，11 章）
  - `docs/taiji-quant-change-summary.md`（全量变更汇总）
- **文件数**: 2
- **新增行数**: ~600
- **删除行数**: 0
- **功能描述**：核心参考文档。代码地图覆盖 taiji-quant 全部 22+ crate（11 章 556 行），变更汇总记录全量变更。
- **验收标准**：
  1. 代码地图准确描述 crate 结构和关系
  2. 变更汇总与实际 diff 一致
- **依赖**: 无
- **优先级**: P2

---

### Wave 8: 死代码标记 & 杂项

> 说明：不直接影响功能的变更，但需要记录和标记。

---

#### R-8.1: DEAD CODE：Warden/RBAC-Poke 标记 + 文档

- **文件清单**：
  - `src/crates/assembly/core/src/agentic/warden/mod.rs`（869 行，100% 死代码）
  - `src/crates/assembly/core/src/agentic/warden/poisson.rs`（240 行，100% 死代码）
  - `src/crates/assembly/core/src/agentic/warden/punishment_executor.rs`（485 行，100% 死代码）
  - `src/crates/assembly/core/src/agentic/warden/SKILL.md`
  - `src/crates/execution/tool-contracts/src/poke.rs`（408 行，100% 死代码）
  - `src/crates/assembly/core/src/agentic/tools/restrictions.rs`（部分，Warden/PunishmentExecutor 角色死代码）
  - `src/crates/assembly/core/tests/rbac_poke_integration.rs`（675 行，仅测试用）
- **文件数**: 7
- **新增行数**: ~2,152
- **删除行数**: 0（保留代码，仅标记）
- **功能描述**：Warden/PunishmentExecutor/Poke 体系约 2,152 行代码处于 100% 死代码状态。此 PR **不删除代码**，而是在每个文件的模块头部添加 `// DEAD CODE: not wired into production pipeline` 标记，并更新相关文档说明死代码范围。RBAC 权限模板中 `AgentRole::Warden` / `AgentRole::PunishmentExecutor` 角色也一并标记。
- **验收标准**：
  1. 所有死代码文件标注清晰
  2. 文档记录死代码范围和原因
  3. 编译不受影响
- **依赖**: 无
- **优先级**: P3

---

#### R-8.2: 修复：taiji-llm chat_stream() todo!() panic

- **文件清单**：
  - `src/crates/taiji/taiji-llm/src/provider/bitfun.rs`（`chat_stream()` 中的 `todo!()` 宏）
- **文件数**: 1
- **新增行数**: ~5
- **删除行数**: ~5
- **功能描述**：将 `taiji-llm/provider/bitfun.rs` 中 `chat_stream()` 的 `todo!()` 宏替换为 `Err("not implemented".into())` 以防止运行时 panic。`todo!()` 宏会在执行到该路径时触发 panic，应返回 Err 以优雅处理。
- **验收标准**：
  1. 调用未实现的 `chat_stream()` 返回 `Err` 而非 panic
- **依赖**: R-4.3
- **优先级**: P1

---

#### R-8.3: 运维脚本更新（relay deploy + 编译警告 + sync-upstream 审计）

- **文件清单**：
  - `src/apps/desktop/src/api/browser_api.rs`（修复编译警告）
  - `src/apps/desktop/src/api/skill_api.rs`（修复编译警告）
  - `src/apps/desktop/src/lib.rs`（修复编译警告）
  - `src/apps/cli/src/self_update.rs`（修复编译警告）
  - `scripts/sync-upstream.ps1`（上游同步脚本）
  - `scripts/gen-plan.py`（计划文档生成工具）
  - `scripts/embed-server.py`（本地 embedding 服务）
  - `scripts/test-onnx.py`（ONNX 测试脚本）
- **文件数**: 6
- **新增行数**: ~200
- **删除行数**: ~100
- **功能描述**：杂项运维更新。(1) 修复上游 sync 后的编译警告（unused import 等）；(2) 审计 `sync-upstream.ps1` 脚本的覆盖行为（历史上曾用 robocopy /E /XO 覆盖自定义改动）；(3) 更新 embed-server.py 和 test-onnx.py 等脚本。
- **验收标准**：
  1. 编译警告消除
  2. sync-upstream.ps1 有清晰的冲突检测说明
- **依赖**: 无
- **优先级**: P2

---

#### R-8.4: 讨论组归档（0x9 方案讨论记录）

- **文件清单**：
  - `discussion-group/` 目录下约 30 个文件（00-会议召集.md → final-solution.md）
- **文件数**: 30
- **新增行数**: ~2,500
- **删除行数**: 0
- **功能描述**："0x9 方案讨论"的多轮迭代记录归档。包括方案提案、交叉回应、立场分歧、互评审查等。尚未被任何审计报告覆盖，此 PR 仅做归档标记。
- **验收标准**：
  1. 归档索引页列出全部讨论文件
  2. 无内容修改
- **依赖**: 无
- **优先级**: P3

---

## 附录 A：已知 Bug 映射

| Bug ID | 描述 | 等级 | 相关 R-ID | 状态 |
|--------|------|:----:|:---------:|:----:|
| G-01 | Cancelled 状态映射错误 | P0 | R-1.2 | 待修复 |
| G-02 | L3 agentic_api 128K 未修复 | P0 | R-1.2 | 待修复 |
| G-03 | Task spawn 路径 `let _ =` 静默吞错 | P1 | R-2.3 | 待修复 |
| G-04 | D6 workspace 路径不一致 | P1 | R-2.3 | 待修复 |
| G-05 | 前端 128K 默认值未同步 | P0 | R-1.3 | 待修复 |
| G-06 | Goal Chain UI 3 个 Bug | P2 | R-3.2 | 待修复 |
| G-07 | Subagent 可见性 2 个 Bug | P1 | R-3.3 | 待修复 |
| G-08 | taiji-llm chat_stream() todo!() panic | P1 | R-8.2 | 待修复 |

## 附录 B：死代码清单

| 模块 | 文件 | 行数 | 死代码状态 | 相关 R-ID |
|------|------|:----:|:----------:|:---------:|
| Warden 守卫体系 | `warden/mod.rs` | 869 | 100% 未接入 | R-8.1 |
| Poisson 调度器 | `warden/poisson.rs` | 240 | 100% 未接入 | R-8.1 |
| 惩罚执行器 | `warden/punishment_executor.rs` | 485 | 100% 未接入 | R-8.1 |
| Warden Skill 文档 | `warden/SKILL.md` | — | 100% 未接入 | R-8.1 |
| Poke DTO + 验证器 | `tool-contracts/src/poke.rs` | 408 | 100% 未接入 | R-8.1 |
| RBAC+Poke 集成测试 | `rbac_poke_integration.rs` | 675 | 仅测试用 | R-8.1 |
| RBAC 权限模板（部分） | `restrictions.rs` | — | 半活 | R-8.1 |
| **合计** | | **~2,677** | | |

## 附录 C：建议合并顺序

```
Wave 0 (基础基座) ─────────────────────────────────────────┐
  R-0.1 → R-0.2 (构建配置 → 产品定义)                       │
  R-0.3 (上游组件恢复) ─────── 可并行                         │
                                                            ▼
Wave 1 (核心修复) ─────────────────────────────────────────┐
  R-1.1 (Windows 兼容) ─────── 可并行                       │
  R-1.4 (前端 Bug) ─────────── 可并行                       │
  R-1.2 (P0 Bug) → R-1.3 (前端 128K 同步)                   │
                                                            ▼
Wave 2 (Session 控制面) ───────────────────────────────────┐
  R-2.1 (R-003 核心) → R-2.2 (R-004 核心) → R-2.3 (遗留缺口)│
                                                            ▼
Wave 3 (前端 Session UI) ──────────────────────────────────┐
  R-3.1 (Session 树 UI)                                    │
  ├─→ R-3.2 (Goal Chain) ← 并行分支                         │
  └─→ R-3.3 (Subagent 可见性) ← 并行分支                    │
  R-3.4 (IPC 增强) ────────── 可延迟                         │
                                                            ▼
Wave 4 (量化引擎核心) ─────────────────────────────────────┐
  R-4.1 (核心引擎)                                          │
  ├─→ R-4.2 (计算指标) ← 可并行                              │
  └─→ R-4.3 (策略决策) ← 可并行                              │
                                                            ▼
Wave 5 (量化扩展) ─────────────────────────────────────────┐
  依赖 R-4.1，各 PR 可并行：                                 │
  R-5.1 (回测执行) │ R-5.2 (内容发布) │ R-5.3 (监控知识)    │
  R-5.4 (基础设施)                                           │
                                                            ▼
Wave 6 (CLI & Skills) ────────────────────────────────────┐
  R-6.1 (CLI TUI) ──────── 独立，可随时合入                 │
  R-6.2 (Skills 全局) ──── 依赖 R-2.1                       │
                                                            ▼
Wave 7 (品牌 & 文档) ──────────────────────────────────────┐
  R-7.1 (品牌) │ R-7.2 (治理) │ R-7.3 (知识库)             │
  R-7.4 (计划) │ R-7.5 (代码地图) ← 全可并行                 │
                                                            ▼
Wave 8 (死代码 & 杂项) ────────────────────────────────────┐
  R-8.1 (死代码标记) │ R-8.2 (panic 修复)                  │
  R-8.3 (运维) │ R-8.4 (讨论组) ← 全可并行                   │
```

---

*本文档由 AI Agent 基于 git diff 统计、审计报告和功能分类映射生成。覆盖 499 个变更文件，+69,756 / -3,446 行。*