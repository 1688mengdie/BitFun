# 修复报告：Cargo+Cfg门

## 概述
对所有PR分支的 `Cargo.toml` 和 `#[cfg(feature = "taiji")]` 保护完整性进行检查和修复。

## 检查结果

### 1. Root Cargo.toml 完整性

| 检查项 | 预期 | 结果 |
|--------|------|------|
| BitFun members 数量 | 36 | ✓ 全部36个 |
| Taiji members 数量 | 21 | ✓ 全部21个 |
| `license = "MIT"` | 存在 | ✓ |
| 13个taiji workspace deps | 存在 | ✓ |
| `exclude = ["BitFun-Installer/src-tauri"]` | 存在 | ✓ |

### 2. 各分支Cargo.toml问题汇总

| 分支 | 初始状态 | 问题类型 | 修复方式 | 当前状态 |
|------|---------|---------|---------|---------|
| feat/pr-01-session-tree | 仅有21个taiji members | **Group A**: 缺失36个BitFun members | 插入完整members块+exclude | ✓ 57 members |
| feat/pr-02-rbac-poke-warden | 仅有21个taiji members | **Group A**: 缺失36个BitFun members | 插入完整members块+exclude | ✓ 57 members |
| feat/pr-03-coordination-tools | 仅有21个taiji members | **Group A**: 缺失36个BitFun members | 插入完整members块+exclude | ✓ 57 members |
| feat/pr-04-hook-integration | 仅有21个taiji members | **Group A**: 缺失36个BitFun members | 插入完整members块+exclude | ✓ 57 members |
| feat/pr-05-frontend-session-tree | 仅有36个BitFun members | **Group B**: 缺失21个taiji members + license + 13 taiji deps | 追加taiji members/license/deps | ✓ 57 members |
| feat/pr-06-legion-frontend | 仅有36个BitFun members | **Group B**: 缺失21个taiji members + license + 13 taiji deps | 追加taiji members/license/deps | ✓ 57 members |
| feat/pr-07-encoding-fixes | 仅有36个BitFun members | **Group B**: 缺失21个taiji members + license + 13 taiji deps | 追加taiji members/license/deps | ✓ 57 members |
| feat/pr-08-taiji-engine-core | 仅有21个taiji members | **Group A**: 缺失36个BitFun members | 插入完整members块+exclude | ✓ 57 members |
| feat/pr-09-taiji-remaining | 仅有21个taiji members | **Group A**: 缺失36个BitFun members | 插入完整members块+exclude | ✓ 57 members |

### 3. #[cfg(feature = "taiji")] 覆盖率检查

对BitFun核心改动文件的检查结果：

- **新增模块文件**：Taiji crates (`src/crates/taiji/*`) 是独立的workspace成员，不需要 `#![cfg(feature = "taiji")]`
- **已有文件中的新增字段/方法**：全部使用 `#[cfg(feature = "taiji")]` 或 `#[cfg(not(feature = "taiji"))]` 保护 ✓
- **测试代码**：使用 `#[cfg(feature = "taiji")]` 保护 ✓

检查覆盖的BitFun核心文件（18个）：
- `assembly/core/` 相关文件：coordinator.rs, coordination_store.rs, background_outcomes.rs, session_control_tool.rs, tool_pipeline.rs, restrictions.rs, tool_context_runtime.rs, warden/mod.rs, warden/poisson.rs, warden/punishment_executor.rs, tests/rbac_poke_integration.rs, config/types.rs
- `contracts/` 相关文件：core-types/src/lib.rs, core-types/src/session_tree.rs, events/src/agentic.rs, events/src/frontend_projection.rs, runtime-ports/src/lib.rs
- `execution/` 相关文件：agent-runtime/src/session.rs, agent-runtime/src/session_control.rs, tool-contracts/src/framework.rs, tool-contracts/src/lib.rs, tool-contracts/src/poke.rs
- `services/` 相关文件：services-core/src/session/mod.rs

**未发现未保护的taiji代码引用。** 唯一未受cfg保护的"taiji"引用是一条注释（tool_pipeline.rs:959），无需处理。

## 修复详情

### Group A（缺失BitFun members）
对 pr-01, pr-02, pr-03, pr-04, pr-08, pr-09：
1. 在 `members = [` 后插入36个BitFun成员路径
2. 在members块后添加 `exclude = ["BitFun-Installer/src-tauri"]`

### Group B（缺失taiji内容）
对 pr-05, pr-06, pr-07：
1. 在members块末尾添加21个taiji成员路径（`# taiji-quant 量化引擎 crates` 注释开头）
2. 在 `[workspace.package]` 添加 `license = "MIT"`
3. 在 `url = "2"` 后添加13个taiji workspace依赖

## 验证结果
所有9个PR分支的 Cargo.toml 均达到标准：
- 57个member引用（36 BitFun + 21 taiji）
- 包含 taiji-quant 注释
- 包含 license = "MIT"
- 包含全部13个taiji workspace依赖（含 crossbeam）
