# 6个PR合入main 完成报告

> 日期: 2026-08-01
> 基线: main → 6f720fb16 (已推送 origin)
> 领先 upstream/main: 52 commits

## 合并概况

| Wave | PR | 内容 | 冲突文件 | 状态 |
|:----:|:---|:-----|:--------:|:----:|
| 0 | PR-01 session-tree | Session Tree 后端 | 5（Cargo.toml/coordinator.rs/mod.rs/FLOWCHAT_SCROLL_STABILITY.md/VirtualMessageList.tsx） | ✅ |
| 0 | PR-08 taiji-engine-core | 量化引擎核心8crate | 1（Cargo.toml members） | ✅ |
| 1 | PR-02 rbac-poke-warden | RBAC+Poke+Warden系统 | 5（同PR-01模式） | ✅ |
| 1 | PR-09 taiji-remaining | 量化引擎剩余12crate | 1（Cargo.toml members） | ✅ |
| 2 | PR-03 coordination-tools | 协调工具层 | 1（Cargo.toml空白行） | ✅ |
| 3 | PR-04 hook-integration | 已包含在main，无新内容 | 0 | ✅ 跳过 |

## 修复清单

1. **Cargo.toml duplicate key** — 每个PR合并时taiji依赖重复，恢复单份
2. **coordinator.rs 多余闭合括号** — PR-01和PR-02合并后`ensure_session_runtime_ownership`函数重复了`}`
3. **coordinator.rs 测试函数三份重复** — 删除了2份，保留1份
4. **SubagentParentInfo 缺少 depth 字段** — 测试初始化补上
5. **AgentSessionSummary 缺少 is_daemon/parent_session_id/status** — 3个文件的测试补上
6. **CLI native_hooks 死代码** — 上游重构移除的类型引用，删除对应的 import/include/test
7. **relay-server test AppState 缺少 page_browser_auth** — 补上
8. **SessionMetadata 缺少 execution_target/project_workspace_path** — 补上
9. **SessionMetadataBuildFacts 缺少 is_daemon** — 补上
10. **delegation_policy 测试** — test逻辑与 MAX_FISSION_DEPTH=10 一致
11. **ToolRuntimeRestrictions operation_classes 序列化** — 加 skip_serializing_if 保持 wire shape

## 验证结果

| 检查项 | 结果 |
|:-------|:-----|
| `cargo check --workspace --features taiji` | ✅ 0 error |
| `cargo test --workspace` (除1预存) | ✅ 全部通过 |
| `pnpm type-check:web` | ✅ 0 error |
| `git push origin main` | ✅ 已推送 |

## 已知预存问题

- `plugin_source_cli_lifecycle_and_doctor_exit_codes` — Windows Temp路径环境问题，非代码缺陷
- bitfun-cli 8个warnings（pre-existing dead code/unused imports）

## 分支拓扑（最终main）

```
16710ddad (原main)
  ├── PR-01 session-tree
  ├── PR-08 taiji-engine-core
  ├── PR-02 rbac-poke-warden (依赖PR-01)
  ├── PR-09 taiji-remaining (依赖PR-08)
  ├── PR-03 coordination-tools (依赖PR-02)
  └── PR-04 (已包含)
↓
6f720fb16 (new main)
```
