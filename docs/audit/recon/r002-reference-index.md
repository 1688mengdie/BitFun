# R-002 侦察报告：参考材料索引

## 概述
编列所有已有的中间产物、审计报告、规划文档、讨论记录，建立完整索引。

---

## A. Recon 输出（docs/audit/recon/）

| 文件 | 行数 | 核心内容 | 相关领域 |
|------|:----:|----------|----------|
| `docs/audit/recon/rid-matrix.md` | 1,298 | R-ID 矩阵：功能 PR 拆分规划，覆盖 499 文件，8 个 Wave，40+ PR | 全量审计 |
| `docs/audit/recon/r001-manifest.md` | 215 | R-001 侦察报告：合并基线 + 全量 Diff 文件清单 | 基线稽核 |
| `docs/audit/recon/r002-reference-index.md` | 本文件 | R-002 侦察报告：参考材料索引 | 元索引 |

---

## B. 审计报告

`docs/audit/` 目录下除 `recon/` 外无其他审计报告文件。历史上 R-003/R-004 会话产生的审计产物（`_recon_category_map.md`、`r003-audit.md`、`r004-dispatch.md` 等）存于 `docs/plans/` 中，未以审计报告形式单独归档。

**状态**: 无独立审计报告子目录。部分审计内容嵌入在规划文档中。

---

## C. 规划文档（docs/plans/）

### 已有（workspace 当前 HEAD，7 文件）
| 文件 | 说明 |
|------|------|
| `docs/plans/computer-use-refactor-plan.md` | Computer Use 重构计划 |
| `docs/plans/core-decomposition-completed.md` | Core 分解完成报告 |
| `docs/plans/core-decomposition-plan.md` | Core 分解计划 |
| `docs/plans/desktop-window-fullscreen-plan.md` | 桌面窗口全屏计划 |
| `docs/plans/edit-constraint-guard-plan.md` | Edit 约束保护计划 |
| `docs/plans/opencode-extension-compatibility-plan.md` | OpenCode 扩展兼容性计划 |
| `docs/plans/product-architecture-evolution-plan.md` | 产品架构演进计划 |

### 新增（baseline-clean 中，16 文件，存于 git 但未出现在当前 workspace HEAD）
| 文件 | 说明 |
|------|------|
| `docs/plans/phase-rbac-poke-dispatch-prompts.md` | RBAC Poke 调度提示词 |
| `docs/plans/phase-rbac-poke-plan.md` | RBAC Poke 实施计划 |
| `docs/plans/phase-rbac-poke-type-contract.md` | RBAC Poke 类型契约 |
| `docs/plans/product-cli-mcp-acp-dispatch.md` | CLI MCP/ACP 调度 |
| `docs/plans/product-cli-mcp-acp-plan.md` | CLI MCP/ACP 计划 |
| `docs/plans/product-cli-mcp-acp-types.md` | CLI MCP/ACP 类型 |
| `docs/plans/r003-audit-v1.1.md` | R-003 审计 v1.1 |
| `docs/plans/r003-audit.md` | R-003 审计 |
| `docs/plans/r003-design.md` | R-003 设计 |
| `docs/plans/r003-dispatch.md` | R-003 调度 |
| `docs/plans/r003-handoff.md` | R-003 交接 |
| `docs/plans/r003-requirements.md` | R-003 需求 |
| `docs/plans/r004-design.md` | R-004 设计 |
| `docs/plans/r004-dispatch.md` | R-004 调度 |
| `docs/plans/r004-progress-final.md` | R-004 最终进度 |
| `docs/plans/r004-requirements.md` | R-004 需求 |

---

## D. 知识库

`docs/knowledge-base/` 目录在 **当前 workspace HEAD 上不存在**。该目录下的所有文件（50+ 文件，含 bitfun 与 taiji crate 文档、功能说明、引用索引）属于 baseline-clean 分支的新增内容，尚未合并到 workspace 中。

**状态**: 仅在 baseline-clean 分支的 git diff 中可见，不位于当前工作目录。

---

## E. 讨论记录（discussion-group/）

`discussion-group/` 目录在 **当前 workspace HEAD 上不存在**。包含 30+ 份方案讨论、互评、立场收敛、回应文档，全部为 baseline-clean 新增内容。

**状态**: 仅在 baseline-clean 分支的 git diff 中可见。

---

## F. 其他参考文件

| 文件 | 说明 | 位置 |
|------|------|------|
| `docs/code-map-taiji-quant.md` | 量化代码地图（556 行） | baseline-clean 新增 |
| `docs/taiji-quant-change-summary.md` | 变更总结（325 行） | baseline-clean 新增 |
| `docs/legal/upstream-authorization.md` | 上游授权声明 | baseline-clean 新增 |
| `ACKNOWLEDGMENTS.md` | 致谢 | baseline-clean 新增 |
| `CODE_OF_CONDUCT.md` | 行为准则 | baseline-clean 新增 |
| `CHANGELOG.md` | 变更日志 | baseline-clean 新增 |
| `PR-BODY.md` | PR 正文模板 | baseline-clean 新增 |
| `Git提交规范.md` | 中文 Git 规范 | baseline-clean 新增 |
| `Rust工具链配置建议.md` | Rust 工具链建议 | baseline-clean 新增 |
| `代码审查标准_Checklist.md` | 代码审查清单 | baseline-clean 新增 |
| `代码审查流程文档.md` | 代码审查流程 | baseline-clean 新增 |
| `技术债务治理策略.md` | 技术债务治理 | baseline-clean 新增 |

---

## G. 已有产物状态

| 产物 | 状态 |
|------|------|
| `docs/audit/recon/rid-matrix.md` | **已持久化**（1,298 行，含概览、详细、附录 A/B/C/D） |
| `docs/audit/recon/r001-manifest.md` | **本次创建** |
| `docs/audit/recon/r002-reference-index.md` | **本次创建** |
| `docs/audit/recon/r003-gbrain-insights.md` | **本次创建** |
| baseline-clean git 分支 | **已持久化**（squash commit `2d0599e60`） |
| `_recon_category_map.md`（功能分类映射） | 未持久化到磁盘，仅存在于 ACR 会话记忆 |
| `r001-diff-files.txt`（原始 diff 输出） | 未持久化到磁盘 |
| `r003-gbrain-insights.md`（原始版） | 未持久化到磁盘 |
| `discussion-group/` 全部文件 | 仅在 git 中，未出现在当前 workspace HEAD |
| `docs/knowledge-base/` 全部文件 | 仅在 git 中，未出现在当前 workspace HEAD |
| `docs/plans/` 中 baseline-clean 新增的 16 文件 | 仅在 git 中，未出现在当前 workspace HEAD |

**结论**: 大部分审计中间产物未持久化到磁盘。仅有 `rid-matrix.md` 和 `baseline-clean` git 分支存活。本次任务补充了 `r001-manifest.md`、`r002-reference-index.md` 和 `r003-gbrain-insights.md` 以弥补空白。
