# LVPA 多阶段门禁管线设计方案

> 版本：v1.0 | 日期：2026-07-30
> 设计者：Dev Pipeline Orchestrator
> 对应架构：架构总纲 §14 六阶段 SOP 管线 + G0-G3 四级门禁 + DoD

---

## 一、设计目标

将架构总纲 §14 定义的 Phase 1-6 研发流程 + G0-G3 四级门禁 **自动化落地**为 CI/CD 管线，实现：

1. **Phase 自动路由** — 每个 PR/commit 自动匹配当前 Phase，触发对应门禁
2. **G 级硬阻断** — G1/G2/G3 门禁失败直接阻断合并，无 `continue-on-error`
3. **DoD 自动化校验** — 8 项 Done 条件中可自动化的 6 项全部纳入管线
4. **taiji-private crates 合并** — 修复路径依赖，纳入统一门禁
5. **Layer 1 性能红线** — benchmark 回归 ≥20% 阻断合并

---

## 二、门禁管线总览

```
                ┌─────────────────────────────────────────────────────┐
                │              LVPA Multi-Stage Gate Pipeline          │
                ├─────────────────────────────────────────────────────┤
                │                                                      │
Phase 1 ─────── │  req-label / req-comment → G0 门禁                   │
需求定稿         │  仅通知，不阻塞                                        │
                │                                                      │
Phase 2 ─────── │  design-doc / adr → G1 门禁                          │
方案设计         │  方案文件合规检查（存在性 + 格式）                      │
                │                                                      │
Phase 3 ─────── │  PR 提交 → Git Hooks + PR CI → G1 门禁              │
测试编写         │  cargo test --no-run 验证测试可编译                   │
                │                                                      │
Phase 4 ─────── │  Push CI → G1 + G2 门禁                             │
实现交付         │  全量构建 + 单元测试 + 集成测试 + clippy              │
                │                                                      │
Phase 5 ─────── │  Merge Gate → G2 + G3 门禁                          │
验证合并         │  性能基准 + Code Review + 安全审计 + 最终批准         │
                │                                                      │
Phase 6 ─────── │  Post-merge → 复盘归档自动化                         │
复盘归档         │  R-ID 闭环 + gbrain 更新 + 标签管理                  │
                │                                                      │
                └─────────────────────────────────────────────────────┘
```

---

## 三、工作流设计（6 个 GitHub Actions）

### 3.1 `gate-0-phase-ready.yml` — G0 需求定稿门禁

**触发**：PR opened / issue labeled `phase:1`

```
门禁检查：
  ┌─ PR 描述是否包含需求文档链接？ [check: PR body regex]
  ┌─ 是否有验收标准章节？        [check: PR body regex]
  ┌─ label 是否包含 'phase:1'？  [check: label presence]
  通过 → 自动 label 'gate:0-passed'
  不通过 → comment 提示缺失项，label 'gate:0-blocked'
  等级：G0（仅通知，不阻塞合并）
```

### 3.2 `gate-1-design-check.yml` — G1 方案设计门禁

**触发**：label `phase:2` added / PR labeled `needs-design-review`

```
门禁检查：
  ┌─ 方案文档存在性                   [check: docs/architecture/*.md]
  ┌─ 4 问题减法分析（如果适用）          [check: 减法分析.md 存在]
  ┌─ 接口契约（Rust trait / TS interface）[check: 文件中含 trait/interface/type]
  ┌─ ADR 文件（架构决策记录）             [check: docs/adr/*.md]
  通过 → label 'gate:1-passed'
  不通过 → comment 缺失文档，label 'gate:1-blocked'
  等级：G1（不阻塞 CI 但阻塞 Phase 推进）
```

### 3.3 `ci.yml` — G1 测试 + 构建门禁（强化现有 `taiji-ci.yml`）

**触发**：push / PR（非 draft）

这是现有 `taiji-ci.yml` 的强化版，改为 **硬阻断模式**：

```
on:
  pull_request:
    branches: [main]
    paths-ignore: ['**/*.md', 'png/**', 'assets/**']
  push:
    branches: [main]
    
concurrency: group=lvpa-ci-${{ github.ref }}, cancel-in-progress=true

job-1: rust-fmt           # 格式检查 → 失败阻断
job-2: rust-clippy        # clippy -D warnings → 失败阻断
job-3: rust-build-taiji   # taiji-* crate 编译检查 → 失败阻断
job-4: rust-unit-test     # cargo test -p taiji-* → 失败阻断
job-5: rust-integration   # cargo test --test '*integration*' → 失败阻断
job-6: frontend-build     # TS 类型检查 + lint + build → 失败阻断
job-7: python-quant       # Python 语法 + lint → 失败阻断
job-8: private-crates     # taiji-private 构建 → 失败阻断（修复后）

门禁等级：G1
阻断规则：任何 job 失败 → CI 红色，block merge
```

**与现有 `taiji-ci.yml` 的核心差异**：

| 项目 | 现有 | 设计目标 |
|:-----|:-----|:---------|
| `continue-on-error` | 大量存在 | **删除**，改为硬阻断 |
| `|| echo "WARN"` | 到处是 | **删除**，失败即报错 |
| 测试覆盖 | 部分 crate | **全部 taiji-* crate** |
| 私库检查 | `continue-on-error: true` | **纳入 workspace** 或显式路径 |

### 3.4 `benchmark.yml` — G2 性能门禁（强化现有 `taiji-bench.yml`）

**触发**：PR labeled `benchmark` / push 修改核心 crate / 定时

现有 `taiji-bench.yml` 已有 Criterion + critcmp + PR comment 结构，需增加：

```
新增：
  ┌─ 回归阈值门禁（硬性）：
  │   ├─ BarGenerator 吞吐 < 100,000 ticks/s → FAIL
  │   ├─ DAG 拓扑排序（50 节点）> 1ms → FAIL
  │   ├─ StateStore 读写 < 500,000 ops/s → FAIL
  │   ├─ Pipeline 单 tick 处理 > 10μs → FAIL
  │   └─ 任一基准回归 ≥ 20% → block merge
  │
  └─ 阈值配置化：
      └─ .github/bench-thresholds.yml（可版本化管理）

阻断规则：
  回归 ≥ 20% → PR status check 'benchmark/regression' = failure
  回归 < 20% → PR status check 'benchmark/regression' = success
  无基准 → warning（不阻断）

门禁等级：G2
```

### 3.5 `security.yml` — G2 安全门禁（强化现有 `taiji-security.yml`）

**触发**：push / PR / 定时

现有 `taiji-security.yml` 结构良好，需强化：

```
强化项：
  ┌─ cargo-deny：拆除 'log-level: warn' → fail on policy violation
  ┌─ cargo-audit：已 blocking → 保持
  ┌─ secret-scan：truffleHog 已 fail → 保持；常见模式检查改为硬阻断
  ┌─ npm-audit：拆除 '|| echo "audit:warn"' → 仅高危阻断
  └─ trivy：HIGH/CRITICAL → 上传 SARIF + 阻断

门禁等级：G2
阻断规则：CRITICAL 漏洞 → block merge；HIGH 漏洞 → PR label 'security:high'
```

### 3.6 `post-merge-report.yml` — G3 验收 + Phase 6 复盘自动化

**触发**：push to main

```
自动化任务：
  ┌─ R-ID 闭环检测：扫描 merge commit message 中的 R-ID
  │   └─ 匹配 → 自动 comment 'R-ID closed in <commit>'
  │   └─ 不匹配 → issue 'R-ID not found in merge commit'
  │
  ┌─ 功德簿记录：commit 信息自动提取 → 写入 ledger 元数据
  │
  ┌─ gbrain 知识回流触发：
  │   └─ 新增文档/接口变更自动触发 gbrain reindex
  │
  ┌─ 版本标签管理：
  │   └─ 检测 version bump → 自动打 tag + release draft
  │
  门禁等级：G3（不阻塞运行时，阻塞 DoD 闭环）
```

---

## 四、G 级门禁矩阵

| 门禁 | 等级 | 自动化程度 | 阻断力 | 对应 Phase | 对应 CI Job |
|:-----|:-----|:-----------|:-------|:-----------|:------------|
| G0 | 信息级 | 全自动 | 不阻断 | Phase 1→2 | `gate-0-phase-ready` |
| G1 | 检查级 | 全自动 | 硬阻断 | Phase 2→4 | `ci.yml` + `gate-1-design-check` |
| G2 | 评审级 | 自动+人工 | 硬阻断 | Phase 4→5 | `benchmark.yml` + `security.yml` + Code Review |
| G3 | 验收级 | 半自动 | 软阻断 | Phase 5→6 | `post-merge-report` + 主人确认 |

### 阻断传播链

```
G0 通过 → G1 通过 → G2 通过 → G3 通过 → Merge
   │          │          │          │
   ▼          ▼          ▼          ▼
 通知即可  硬阻断CI   Code Review   主人确认
            失败不合并   + 基准门禁     + 归档自动化
                       + 安全门禁
```

---

## 五、Phase 到 Git 状态映射

| Phase | Git 状态 | 分支策略 | 门禁激活 |
|:------|:---------|:---------|:---------|
| 1 需求定稿 | Issue + Label | — | G0 |
| 2 方案设计 | 方案 PR（`design/` 分支） | `design/<module>-<feature>` | G1 |
| 3 测试编写 | 测试 PR（`test/` 分支） | `test/<crate>-<feature>` | G1 |
| 4 实现交付 | 功能 PR（`feat/` 分支） | `feat/<crate>-<feature>` | G1 + G2 |
| 5 验证合并 | 合并到 main | `main`（保护分支） | G2 + G3 |
| 6 复盘归档 | main 上的 tag/release | `v*` / `release-*` | 自动化触发 |

### 分支保护规则

```
main 分支保护：
  ├── 必须 PR 才能推送
  ├── 必须通过所有 status checks：
  │   ├── ci/rust-fmt
  │   ├── ci/rust-clippy
  │   ├── ci/rust-build-taiji
  │   ├── ci/rust-unit-test
  │   ├── ci/rust-integration
  │   ├── ci/frontend-build
  │   ├── ci/python-quant
  │   ├── ci/private-crates
  │   ├── security/cargo-deny
  │   ├── security/cargo-audit
  │   ├── security/secret-scan
  │   ├── security/npm-audit
  │   └── benchmark/regression（core crate 变更时）
  ├── 必须至少 1 人 Code Review
  ├── 对话必须解决（resolve conversation）
  └── 禁止跳过（skip rule）
```

---

## 六、DoD 自动化校验映射

| DoD 项 | 自动化方式 | 门禁 |
|:-------|:----------|:-----|
| □ 接口契约已定稿 | `gate-1-design-check` 检查契约文件 | G1 |
| □ 测试用例已编写并通过 | CI `rust-unit-test` + `rust-integration` | G1 |
| □ 实现代码已交付并通过审计 | CI 全通 + Code Review 通过 | G2 |
| □ CI 管线全绿 | 所有 status checks = pass | G1+G2 |
| □ Code Review 通过 | GitHub Review 状态检查 | G2 |
| □ 性能验收达标 | `benchmark.yml` 回归阈值 | G2 |
| □ R-ID 已闭环归档 | `post-merge-report` 自动检测 | G3 |
| □ 功德簿已更新 | `post-merge-report` 自动触发 | G3 |

---

## 七、taiji-private crates 合并方案

### 7.1 路径修复（P0）

```
方案：将 taiji-private 纳入主 workspace

software/taiji/Cargo.toml 修改：
  [workspace.members] 添加：
    "../taiji-private/taiji-risk"
    "../taiji-private/taiji-thrust"
    "../taiji-private/taiji-magnet"
    "../taiji-private/taiji-dvmi"
```

### 7.2 私库构建门禁

合并后，ci.yml 中 `private-crates-build` job 改为 **硬阻断**（移除 `continue-on-error: true`）：

```yaml
private-crates-build:
  name: Private Crates Build
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v5
      with:
        repository: ${{ github.repository_owner }}/taiji-private
        ref: main
        path: software/taiji-private
    - name: Build private crates
      run: |
        cargo check -p taiji-risk -p taiji-thrust \
                     -p taiji-magnet -p taiji-dvmi
```

### 7.3 私库发布策略

```
公开 crate（taiji-engine/bar/pattern 等）：
  └── crates.io 发布 + git tag

私有 crate（taiji-risk/thrust/magnet/dvmi）：
  └── 仅 git 依赖（path = workspace 内部）
  └── 不发布到 crates.io
  └── 通过 GitHub release 提供 git tag
```

---

## 八、Layer 1 性能红线门禁（基准阈值）

### 8.1 阈值配置

```yaml
# .github/bench-thresholds.yml
thresholds:
  bar_generator_throughput:
    min: 100_000           # ticks/s
    unit: "ticks/sec"
  dag_topological_sort:
    max_ms: 1.0            # 50 nodes
    unit: "ms"
  state_store_ops:
    min: 500_000           # ops/s
    unit: "ops/sec"
  pipeline_tick:
    max_us: 10.0           # single tick, no LLM
    unit: "μs"
  regression_max_pct: 20   # 基准回归上限 %
```

### 8.2 门禁行为

```
benchmark.yml 中新增 job:

job-check-thresholds:
  needs: bench-compare
  steps:
    - 解析 bench-report.txt → 提取 each metric
    - 对比 .github/bench-thresholds.yml
    - 任何指标不达标 → 输出错误信息 + exit 1
    - 回归 ≥ 20% → 输出回归详情 + exit 1
```

---

## 九、实施路线图

| 阶段 | 内容 | 前置依赖 | 工作量估算 |
|:-----|:------|:---------|:----------|
| **Phase A: 基线加固** | 修复 ci.yml 硬阻断模式、删除 continue-on-error | 无 | 小 |
| **Phase B: 私库合并** | 修复 taiji-private 路径依赖、纳入 workspace | A | 中 |
| **Phase C: G0/G1 门禁** | 新建 gate-0 和 gate-1 workflow | A | 小 |
| **Phase D: 基准红线** | 阈值配置化 + benchmark 硬阻断 | B（需要 taiji-engine 可编译） | 中 |
| **Phase E: 安全强化** | 安全扫描硬阻断化 | A | 小 |
| **Phase F: G3/复盘** | post-merge-report workflow | 无 | 中 |

### 优先级排序

```
P0（阻塞，立即执行）：
  1. ci.yml 硬阻断化（删除 continue-on-error）
  2. taiji-private 路径修复
  3. gate-0 需求门禁
  
P1（重要，Phase A-D 完成后）：
  4. gate-1 方案门禁
  5. benchmark 阈值门禁
  6. 安全扫描强化

P2（持续优化）：
  7. post-merge 复盘自动化
  8. 覆盖率门禁（cargo tarpaulin）
  9. 全量集成测试 weekly/nightly
```

---

## 十、不在此方案范围内的事项

1. **移动端 CI** — 待移动端技术选型定稿后加入
2. **容器镜像 CI/CD** — 架构总纲 §13.2 已有定义，与门禁管线正交
3. **发布管线（crates.io）** — 属于发布管理工作流，需要独立设计
4. **Tauri 桌面端 E2E 测试** — 需要硬件环境，不适合作为 PR 门禁
5. **gbrain reindex 触发** — 需要 gbrain 模块就绪后实现

---

## 十一、与现有 CI 的兼容性

| 现有工作流 | 处理方式 | 原因 |
|:-----------|:---------|:------|
| `taiji-ci.yml` | ✅ **改造 → ci.yml** | 核心门禁，硬阻断化 |
| `taiji-bench.yml` | ✅ **改造 → benchmark.yml** | 增加阈值门禁 |
| `taiji-security.yml` | ✅ **改造 → security.yml** | 强化阻断规则 |
| `ci.yml`（BitFun 原有） | ✅ **保留不动** | BitFun 上游 CI，不修改 |
| `desktop-package.yml` | ⏸ **保留不动** | 属于发布管线 |
| `nightly.yml` | ⏸ **保留不动** | 属于发布管线 |
| `cli-package.yml` | ⏸ **保留不动** | 属于发布管线 |
| `release-please.yml` | ⏸ **保留不动** | 属于发布管线 |

---

## 十二、关键验收标准

```
□ ci.yml 所有 job 无 continue-on-error，失败即红
□ taiji-private 4 个 crate 可通过 cargo check --workspace
□ PR 提交流程：创建 → 门禁全自动检查 → 显示 pass/fail
□ benchmark 回归 ≥ 20% → status check = failure
□ G0/G1/G2/G3 四级门禁状态可追踪（PR label + check）
□ DoD 8 项中 6 项可自动化验证
□ 分支保护规则已配置并生效
```
