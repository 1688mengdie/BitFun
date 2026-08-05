# 自定义 Agent（agents/*.md）热更新机制

本文档说明自定义 Agent 配置文件（`agents/*.md`）的加载、缓存与生效时机：
哪些字段修改后立即生效、哪些需要重启、Agent Listing 快照何时刷新。

> 适用版本：基于 `d1c3e630a` 之后的主线（本文撰写时 HEAD 为 `c335b0d6c`）。
> 所有结论均来自源码实测，引用格式 `文件:行号`；未实测的内容明确标注「未实测」。

## 一、读取路径与来源

| 来源 | 路径 | 注册方式 | 热更新 |
| --- | --- | --- | --- |
| 内置 Agent | 编译期代码注册（`builtin_agent_specs` + 工厂 match） | `src/crates/assembly/core/src/agentic/agents/registry/catalog.rs:20-54` | 否（需改代码重编译） |
| 用户自定义 Agent | `<用户数据根>/agents/`，Windows 实测 `C:\Users\<user>\AppData\Roaming\bitfun\agents\` | `src/crates/assembly/core/src/infrastructure/app_paths/path_manager.rs:233-236`（`user_agents_dir()`） | 是 |
| 项目自定义 Agent | `<工作区>/.bitfun/agents/` | `src/crates/execution/agent-runtime/src/custom_agent.rs:34`（`CUSTOM_AGENT_PROJECT_AGENT_SUBDIRS`） | 是（按工作区加载） |
| 外部 Agent（ACP 桥接等） | 运行时注册，非文件 | `src/crates/assembly/core/src/agentic/agents/registry/external.rs:319`（`install_external_subagent_routes`） | 运行时（本文不展开） |

扫描目录的优先级与存在性检查见
`src/crates/execution/agent-runtime/src/custom_agent.rs:334-359`（`custom_agent_possible_dirs`）：
项目目录在前、用户目录在后；同 id 按此优先级去重（`custom_agent.rs:407-417`）。

## 二、加载机制（无缓存，全量重读）

`agents/*.md` 的解析没有文件监听、没有 mtime 缓存 —— 每次加载都是**全量重读磁盘**：

1. `load_custom_agent_definitions`（`src/crates/execution/agent-runtime/src/custom_agent.rs:361-423`）
   遍历目录内所有 `*.md`（`list_custom_agent_markdown_files`，`custom_agent.rs:486-499`），
   解析 front matter（`custom_agent_read_markdown_str`，`custom_agent.rs:546-570`），
   字段：`kind / id / name / description / tools / readonly / review / model / user_context_policy` + 正文 prompt。
2. 校验与归一化（`validate_custom_agent_definition`，`custom_agent.rs:425-472`）：
   非法 tools 过滤、非法 model 回退默认值；**`review: true` 的 subagent 强制 `readonly` 并剔除所有可写工具**。
3. 写入注册表：user 条目先清旧再插入（`src/crates/assembly/core/src/agentic/agents/registry/custom.rs:136-146`），
   project 条目写入工作区维度缓存（`custom.rs:148-166`）。

注册表是**进程级全局单例**（`registry/mod.rs:232-243` `GLOBAL_AGENT_REGISTRY`），
内存中 `id -> AgentEntry` 映射（`registry/mod.rs:54-61`），`AgentEntry` 持有不可变的
`Arc<dyn Agent>` 对象（`definitions/custom/subagent.rs:32-67` 等），字段值在加载时固化。

两个入口的差别是关键：

| 入口 | 行为 | 位置 |
| --- | --- | --- |
| `load_custom_agents(workspace_root)` | **无条件全量重读**并替换 user 条目 | `registry/custom.rs:41-47` |
| `ensure_user_custom_agents_loaded()` | **进程级标志位，首次加载后永久跳过** | `registry/custom.rs:33-38`（标志位 `mod.rs:59`） |

## 三、各字段生效时机

所有字段都来自同一次 front matter 解析，**不存在"部分字段热更新"的区分**；
生效快慢取决于「下一次触发加载的路径」：

| 字段 | 生效时机 | 说明 |
| --- | --- | --- |
| `tools` | **立即**（下一轮对话 / 下一次 spawn） | review 锁剔除在加载时执行（`custom_agent.rs:440-448`）；去掉 `review: true` 后下一次加载即恢复全部工具 |
| `prompt`（正文） | **立即**（下一轮对话） | 每轮对话前重载注册表，`build_prompt` 读新对象（`definitions/custom/subagent.rs:52-54`） |
| `name` / `description` | 立即（对话路径）；延迟（纯前端查看） | description 同时进入 `<available_agents>`（每轮重建）与会话快照 diff |
| `kind` / `id` | 立即（对话路径） | 新增文件：下一次加载即注册；改名 = 删除旧 id + 新增新 id |
| `readonly` / `review` | 立即（对话路径） | 影响 `is_readonly()` / Task 并发判定（`query.rs:222-247`） |
| `model` | 立即（对话路径） | 影响 subagent 默认模型解析（`coordinator.rs:10286-10459` `resolve_fresh_subagent_model_id`） |
| `user_context_policy` | 立即（对话路径） | 加载时解析进 `UserContextPolicy` |
| 内置 Agent 的任何字段 | **需重启** | 编译期注册（`catalog.rs:20-54`） |

「对话路径」= 任何一次用户输入被处理。每一次用户输入都会触发
`load_custom_agents`（见下节触发清单），所以**只要进程里发生过对话，
registry 中的 user 条目就是最新文件内容**。

「纯前端查看」= 只打开/刷新 Agent 列表而没有任何对话活动的场景，见第四节。

## 四、Agent Listing 快照刷新

「Agent Listing」有两层含义，行为不同：

### 4.1 注入模型上下文的 `<available_agents>`（每轮实时重建）

每轮 turn 构建系统提示时，若该会话有 Task 工具，则实时构建可派发 Agent 清单：

- `src/crates/assembly/core/src/agentic/execution/execution_engine.rs:1095-1096` → `TaskTool::build_available_agents_context_section`
- `src/crates/assembly/core/src/agentic/tools/implementations/task/mod.rs:93-96`：**构建前先 `load_custom_agents`（全量重读）**，再查询启用清单
- `task/mod.rs:64-79`：清单内容 = `agent id + description + default_tools`（`<available_agents>`）

结论：**每轮 turn 构建的 listing 一定是最新文件内容**；模型"上一轮看到的旧 listing"
不会被撤回，而是通过下面的 diff 提醒机制告知变化。

### 4.2 会话内快照 diff（AgentListingDiff 提醒）

- 每个会话按 turn 稀疏保存快照 `TurnSkillAgentSnapshot`（`src/crates/execution/agent-runtime/src/skill_agent_snapshot.rs:296-362`），内容 = skills + subagents（id/description/default_tools，`skill_agent_snapshot.rs:23-28`）
- turn 0 保存 baseline；之后每轮 diff 前后快照（`coordinator.rs:3688-3709`）：
  - 变化检测字段：id、description、default_tools（**tools 顺序无关**，`skill_agent_snapshot.rs:206-219`）
  - 有变化 → 注入 `AgentListingDiff` 内部提醒（Added/Changed/Removed Agents，`skill_agent_snapshot.rs:109-141`）+ 保存新 baseline
- 快照在 fork/复制会话时继承 baseline（`coordinator.rs:8834 / 9861 / 10663`）

结论：**会话内模型感知的 Agent 列表更新 = 下一轮 turn 的 diff 提醒**，不是即时全量替换。

### 4.3 前端 Agent 列表（mode 选择器 / subagent 管理页）

- 前端每次打开/挂载都调 API（`AgentAPI.ts:1409-1415` `get_available_modes`、`useAgentsList.ts:195,409` 等）
- 后端命令 `get_available_modes` → `get_modes_info_for_workspace`（`src/apps/desktop/src/api/agentic_api.rs:3418-3421`）→ **只走 `ensure`（进程级标志，首次后跳过），不主动重读磁盘**（`query.rs:148-153`）

结论：**改动 `agents/*.md` 后，如果没有对话活动触发重载，前端列表 API 返回注册表里的旧 user 条目**。
实践中任何一轮对话都会刷新注册表，此后前端列表即最新；否则需要重启进程。
（前端 UI 层是否有本地缓存/刷新策略：未实测）

## 五、触发重载（重新扫描 agents 目录）的完整时机清单

| 触发点 | 时机 | 行为 | 位置 |
| --- | --- | --- | --- |
| `wrap_user_input` | **每一轮用户输入** | 无条件全量重读 | `coordinator.rs:3644-3649` |
| `resolve_primary_agent_for_workspace` | **每次会话创建 / 主 agent 解析** | 无条件全量重读 | `coordinator.rs:1516` |
| `resolve_skill_agent_snapshot` | **每轮 turn 构建快照** | 无条件全量重读 | `skill_agent_snapshot.rs:40-46` |
| `get_enabled_agents`（Task 工具） | **每轮 turn 构建 `<available_agents>`** | 无条件全量重读 | `task/mod.rs:96` |
| `get_subagents_for_query` / `get_subagents_info` | 前端 subagent 列表查询 | ensure（首次后跳过）+ **project 缓存缺失才全量重读** | `query.rs:344-351` |
| `get_modes_info_for_workspace` / `get_available_modes` | 前端 mode 列表查询 | **只 ensure（首次后跳过）** | `query.rs:153` |
| `get_agent_ids_for_session_creation` | 创建会话的 id 清单 | 只 ensure | `query.rs:196` |
| `get_custom_agent_detail` 等编辑路径 | 前端编辑 Agent 配置 | ensure + 全量重读 | `custom.rs:417-421 / 446-449 / 597-600` |
| 工作区切换 | `clear_custom_agents` 清空 project 缓存 | 仅 project 维度 | `custom.rs:267-271` |

注意 `query.rs` 中「project 缓存缺失才全量重读」的调用（`query.rs:31 / 286 / 349 / 451`）
会同时刷新 user 条目（`load_custom_agents` 的实现如此），所以这些路径也能让 user 改动生效。

## 六、已知限制

1. **无文件监听**：不监听 `agents/*.md` 的增删改，全部依赖上述触发点重读；纯查看列表（无对话）场景下 user 改动不生效，需重启进程。
2. **`ensure` 标志进程级**：`user_custom_agents_loaded` 一旦置位永久生效（`custom.rs:33-38`），不会按时间/文件变化重置。
3. **review 锁**：`review: true` 的 subagent 强制 `readonly=true` 并剔除可写工具（`custom_agent.rs:440-448`）；显式写可写工具会触发校验错误（`registry/custom.rs:248-264`）。
4. **id 冲突静默跳过**：与已注册 id 冲突的 user 条目被跳过并打 warning（`custom.rs:140-143`）；project 条目与全局冲突同样跳过（`custom.rs:153-159`）；同 id 去重时项目目录优先于用户目录（`custom_agent.rs:394-417`）。
5. **内置 Agent 不可热更新**：编译期注册（`catalog.rs:20-54`），需改代码重编译。
6. **每次全量重读**：目录文件多时每次加载开销线性增长（无增量/缓存）。
7. **旧快照不撤回**：模型上下文里已注入的旧 `<available_agents>` 不消失，依赖下一轮 `AgentListingDiff` 提醒（`skill_agent_snapshot.rs:109-141`）。

## 七、实测证据摘要

- 用户 agents 目录实测存在：`C:\Users\Administrator\AppData\Roaming\bitfun\agents\`（2026-08-05，含 doc-governance 等 7 个自定义 Agent 文件）
- 工作区 `.bitfun/agents/` 实测不存在（本项目无 project agents）
- 「tools 去 review 锁后 spawn 立即带新工具」的机制：spawn 前 parent 轮已全量重读（`coordinator.rs:3644`）→ 注册表新条目（`custom.rs:136-146`）→ spawn 会话的工具策略读取注册表（`query.rs:75-125` `get_agent_tool_policy` → `default_tools` = `data.tools`，`definitions/custom/subagent.rs:56-58`）

## 八、未实测项

- 前端 UI（mode 选择器 / subagent 管理页）的本地缓存与刷新策略（`useAgentsList.ts` / `useWorkspaceModeCatalog.ts` 未做运行时验证）
- Web UI / 移动端同路径行为
- 外部 Agent（ACP 桥接）的更新时机（`external.rs` 运行时注册，非文件驱动，未展开）
