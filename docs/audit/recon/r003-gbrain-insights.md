# R-003 侦察报告：gbrain 知识库检索与经验提取

## 任务概述
从 gbrain 知识库中检索与当前代码审计、PR 拆分相关的经验教训，为 R-ID 矩阵中的 Bug 和 PR 拆分提供背景参考。

## gbrain think 结果
gbrain 可用（版本 0.42.66.0）并成功运行。查询主题为：审计 BitFun upstream 与 taiji-quant 定制化差异，用于 PR 拆分规划。

## gbrain search 结果
- 第一次搜索因超时而失败
- 第二次搜索成功，获取了相关文档内容
- 关键文件已定位并读取

## 综合结论

### 核心发现

1. **taiji-quant 与 upstream 的差异集中在 5 个领域**:
   - 量化引擎（taiji-engine 及周边 crate）— 全新 crate，约 60% 的新增代码
   - Session 控制面（Coordination, Session Tree, RBAC/Poke）— 对上游既有模块的深层修改
   - 前端增强（Legion Mode, Flow Chat 扩展, 配置重构）— 对上游前端代码的修改与新增组件
   - CLI/工具链（taiji-cli, sync-upstream, gen-plan）— 新增 CLI 应用与开发脚本
   - 治理/文档（知识库、法律声明、讨论记录、品牌替换）— 纯新增文档

2. **Bug 优先级清晰**: 8 个已知 Bug 中有 3 个 P0（Cancelled 状态、L3 128K、前端同步），应优先修复。

3. **依赖链**: 量化引擎内部有强依赖关系（R-4.1 → R-4.2/4.3 → R-4.4），Session 控制面也有线性依赖（R-2.1 → R-2.2 → R-2.3 → R-3.1）。

4. **死代码问题**: Warden/RBAC-Poke 体系（~2,152 行）100% 无调用方，应标记或移除。

## 已知错误清单（从 rid-matrix.md 附录 A 提取）

| Bug ID | 严重度 | 描述 | 归属 R-ID | 状态 |
|:------:|:------:|------|:---------:|:----:|
| 1 | P0 | `SubagentResultStatus` 无 `Cancelled` 变体，取消的子 agent 映射为 `Failed` | R-1.2 | 待修复 |
| 2 | P0 | L3 128K 未修复：`agentic_api.rs` 第 1509 行 `unwrap_or(128128)` 未改为 `1_048_576`（仅 subagent 路径修了） | R-1.1 | 待修复 |
| 3 | P0 | 前端默认值未同步（128K） | R-1.1 | 待修复 |
| 4 | P1 | Task spawn 路径 `let _ =` 静默吞错，应改为 `if let Err(e) = ... log::warn!` | R-1.5 | 待修复 |
| 5 | P1 | D6 workspace 路径不一致：子节点用 `display_workspace`，父节点用 `project_workspace` | R-1.5 | 待修复 |
| 6 | P2 | Goal Chain UI bugs：L0 显示替代 target icon, 多级链空祖先标签, 链未过滤空条目 | R-1.4 | 待修复 |
| 7 | P2 | Subagent 可见性 Bug：前台子 agent 出现在导航面板, 删除后自动恢复 | R-1.4 | 待修复 |
| 8 | P1 | `taiji-llm` `chat_stream()` 中 `todo!()` 宏可能导致运行时 panic | R-5.1 | 待修复 |

## 死代码清单（从 rid-matrix.md 附录 B 提取）

| 模块 | 位置 | 行数 | 归属 R-ID | 说明 |
|------|------|:----:|:---------:|------|
| Warden | `assembly/core/src/agentic/warden/mod.rs` | ~500 | R-8.6 | 100% 无调用方 |
| Poisson 调度 | `assembly/core/src/agentic/warden/poisson.rs` | ~300 | R-8.6 | 100% 无调用方 |
| PunishmentExecutor | `assembly/core/src/agentic/warden/punishment_executor.rs` | ~500 | R-8.6 | 100% 无调用方 |
| Warden SKILL.md | `assembly/core/src/agentic/warden/SKILL.md` | ~200 | R-8.6 | 配套文档，无消费者 |
| Poke tool | `execution/tool-contracts/src/poke.rs` | ~400 | R-8.6 | Poke 体系入口，无调用方 |
| Restrictions 部分 | `execution/tool-contracts/src/restrictions.rs` | ~252 | R-8.6 | 部分代码无调用方 |
| **小计** | | **~2,152** | | |

## 建议

1. **优先处理 Wave 0 基础基座（R-0.1, R-0.2）** — 所有后续 PR 依赖此波次正确合并
2. **先修复 P0 Bug（R-1.2, R-1.1）再推进业务功能** — Bug 修复应作为 Wave 1 的核心
3. **Wave 1 的 R-1.1~R-1.5 可完全并行** — 无交叉依赖
4. **Wave 2 必须串行（R-2.1 → R-2.2 → R-2.3 → R-3.1）** — Session 控制面有强依赖链
5. **Wave 4 量化引擎核心需按 R-4.1 → R-4.2/4.5 → R-4.3 → R-4.4 顺序** — 数据层先于业务逻辑
6. **Wave 8 死代码（R-8.6）建议在合并前标记或移除** — 避免将无用代码带入 upstream
7. **每次 PR 后运行对应验证命令** — 使用 AGENTS.md 中的最小验证表
