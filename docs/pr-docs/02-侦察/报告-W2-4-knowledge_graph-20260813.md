# 报告-W2-4-knowledge_graph-20260813

**执行者**：姬码锋-执行官（W2-4 knowledge_graph 批执行）
**日期**：2026-08-13
**任务**：W2-4 knowledge_graph 全链闭环（同 W1 模式：taiji-lvpa RPC 域 + 开发版工具六处注册面）
**状态**：✅ 完成（三证据齐备：双仓 commit + 测试全绿 + 报告落盘）

---

## 〇、关键判定

**新增域**——开发版无任何策略知识图谱能力（规划 §1.4 grep 实测 0 命中）；与群聊（GroupChatTool）/图谱现有能力（git graph/session tree）零重叠。唯一新依赖：taiji-server Cargo.toml 加一行 `taiji-knowledge-graph` 同仓 path 依赖（petgraph/serde/chrono 已锁，零网络新增，P3 已登记）。

---

## 一、交付物清单

### 源区 taiji-lvpa（master）

| 文件 | 改动 |
|---|---|
| `crates/taiji/taiji-server/src/rpc/kgraph.rs` | **新增**：`kgraph.query`（2-hop 子图）/ `kgraph.search`（grep/semantic/hybrid 三层搜索，默认 hybrid）/ `kgraph.path`（astar 最短路）/ `kgraph.meta`（统计 + 按类别分布 + 可选 BFS 布局）——接入 taiji-knowledge-graph 编译期静态图谱（build.rs 从 7 Agent JSON Schema + golden tick 生成，纯只读）；图谱/布局/语义索引进程级 OnceLock 惰性单例（构建成本一次性）；语义模式缺省 MockEmbeddingService（确定性 mock 嵌入，离线可跑，W2-3 gbrain 先例）+ 空索引回落 grep 降级不报错；未知 id → -32602 明确报错 |
| `crates/taiji/taiji-server/tests/rpc_kgraph_test.rs` | **新增**：10 用例集成测试（真实 build_router + POST /api/rpc）——query 子图非空 + 节点/边结构完整、未知 id -32602、search grep 命中 + 结构、search hybrid 缺省可跑、search 非法 mode -32602、path 两端正确（验收断言）、path 未知端点 -32602、meta 分类和 = node_count（三层次图完整）、meta include_layout 布局返回、缺必填参数 -32602 |
| `crates/taiji/taiji-server/src/rpc/mod.rs` | `pub mod kgraph;` |
| `crates/taiji/taiji-server/src/lib.rs` | `rpc::kgraph::register_kgraph`（register_degraded 之后，注册顺序铁律） |
| `crates/taiji/taiji-server/Cargo.toml` | `taiji-knowledge-graph = { path = "../taiji-knowledge-graph" }`（同仓 path 依赖，零网络新增；taiji-llm 已有声明 L84 复用） |

### 开发版 BitFun（main）

| 文件 | 改动 |
|---|---|
| `src/crates/assembly/core/src/agentic/tools/implementations/quant_tools.rs` | **新增** `QuantKnowledgeGraphTool`（`knowledge_graph` 多 action：query/search/path/meta；复用 `taiji_rpc_call` 9527 通道，零新依赖；**Direct 暴露**——只读纯查询，规划 §二 判定「只读纯函数 → Direct」）+ 2 单元测试（validate 按 action 匹配必填参数 + Direct/只读/并发安全断言） |
| `src/crates/assembly/core/src/agentic/tools/implementations/mod.rs` | 导出 `QuantKnowledgeGraphTool` |
| `src/crates/assembly/core/src/agentic/tools/product_runtime/materialization.rs` | 实例化 `knowledge_graph` |
| `src/crates/assembly/core/src/agentic/tools/restrictions.rs` | Commander 白名单加 `knowledge_graph` |
| `src/crates/execution/tool-provider-groups/src/lib.rs` | **单源真相**：`tool_feature_group` 映射（AgentControl 组）+ core.session tool_names + 展开断言（三处同步） |
| `src/crates/assembly/core/src/agentic/tools/registry.rs` | builtin/readonly 两处 manifest 断言同步（**deferred 不加**——Direct 暴露不进 `get_deferred_tool_names()`，实测 `default_exposure()==Deferred` 才入列） |

> **注册面实证**：六处 = implementations/mod.rs 导出 + materialization.rs 实例化 + restrictions.rs 白名单 + registry.rs manifest 断言 + tool-provider-groups lib.rs（plan 单源真相）。与 W1-5/W2-1 报告一致。

---

## 二、taiji-server RPC 域设计

### kgraph.* — 策略知识图谱（编译期静态数据，纯只读）

```
kgraph.query:
  params: { concept_id: "agent_decision" }   // 必填
  result: { root, node_count, edge_count, nodes: [{id,name,category,description,sources}], edges: [{from,to,relation,weight,label}] }
  // 2-hop 子图（根 + 直接邻居 + 邻居的邻居）；未知 id → -32602

kgraph.search:
  params: { query: "结构", mode?: "grep"|"semantic"|"hybrid", top_k?: 10 }
  result: { mode, results: [{node, score, related_ids}], total[, note] }
  // grep = 关键词匹配（name/description，score 0.4-1.0）
  // semantic = MockEmbeddingService 确定性嵌入 → cosine ANN
  // hybrid = grep 0.3 + semantic 0.7 加权融合重排序（hybrid_rerank）
  // 索引不可用（构建失败）→ 自动回落 grep 附 note，不报错

kgraph.path:
  params: { from_id: "theory_structure", to_id: "data_trend_direction" }  // 必填
  result: { path: [id,...], cost, found }
  // astar 最短路；端节点不存在 → -32602；无路 → {found:false, path:[]}

kgraph.meta:
  params: { include_layout?: bool }   // 缺省 false
  result: { node_count, edge_count, by_category: {concept,strategy,case}, include_layout, layout: [{id,x,y,layer}] }
  // include_layout=false 缺省——layout 为 O(N) 全图 BFS，按需惰性计算
```

- **纯只读**：图谱编译期静态数据（build.rs），无运行时写、无 AppState 新增字段
- **惰性单例**：`OnceLock<KnowledgeGraph>` + `OnceLock<SemanticIndex>` 进程级，构建成本一次性（compute_layout O(N) BFS 同步惰性）
- **语义缺省离线**：MockEmbeddingService（确定性 hash 向量，W2-3 gbrain 同款先例）——真实嵌入由未来 LLM 配置接入，不阻塞基础能力
- **降级不报错**：语义索引构建失败 → search 回落 grep + `note` 字段，能力降级明确登记
- **错误边界**：`query_subgraph`/`path_between` 对未知 id 返回 None → RPC 层转 -32602（规划 §1.4 风险边界）

---

## 三、验证证据

### 3.1 测试（全绿，--jobs 4 分 crate）

| 目标 | 命令 | 结果 |
|---|---|---|
| 源区 taiji-server check | `cargo check -p taiji-server --jobs 4` | ✅ 编译通过 |
| 源区 rpc_kgraph_test | `cargo test -p taiji-server --test rpc_kgraph_test --jobs 4` | **10 passed / 0 failed** |
| 源区 taiji-server lib kgraph 单测 | `cargo test -p taiji-server --lib rpc::kgraph --jobs 4` | **10 passed / 0 failed** |
| 开发版 quant_tools | `cargo test -p bitfun-core --all-features --lib knowledge_graph` | **2 passed / 0 failed**（validate + Direct/只读断言） |
| 开发版 tool_names | `cargo test -p bitfun-core --all-features --lib tool_names_match_registered_contract` | **2 passed / 0 failed**（含 knowledge_graph 名断言） |
| 开发版 registry manifests | `cargo test -p bitfun-core --all-features --lib registry_preserves` | **4 passed / 0 failed**（builtin/deferred/readonly/provider plan） |
| 开发版 restrictions | `cargo test -p bitfun-core --all-features --lib restrictions` | **23 passed / 0 failed** |
| 开发版 tool-provider-groups | `cargo test -p bitfun-tool-packs --jobs 4` | **11 passed / 0 failed** |

### 3.2 9527 真实连通性验证（非 mock，真实 server 进程）

启动 `taiji-server.exe`（监听 127.0.0.1:9527）后真实 HTTP POST：

| 调用 | 参数 | 结果 |
|---|---|---|
| `kgraph.meta` | `{}` | `node_count:57`（concept 10 + strategy 7 + case 40）+ `edge_count:62` ✅ 三层次图完整 |
| `kgraph.query` | `concept_id:"agent_decision"` | 2-hop 子图 **47 节点 + 47 边**（含 concept/strategy/case 三类 + derives_from/uses/contains 关系，weight/label 齐全）✅ |
| `kgraph.search` | `{query:"结构", mode:"grep", top_k:3}` | theory_structure(0.9) / agent_structure(0.9) / data_pivot_structure(0.7) + related_ids ✅ |
| `kgraph.search` | `{query:"结构"}`（缺省 hybrid） | **10 条**，mode=hybrid，语义层召回 grep 未命中的 theory_delta/theory_space 等（三层搜索真实融合）✅ |
| `kgraph.path` | `{from:"theory_structure", to:"data_trend_direction"}` | `path:["theory_structure","agent_structure","data_trend_direction"], cost:2, found:true` ✅ **验收断言通过** |
| `kgraph.query` | `concept_id:"no_such"` | `{"code":-32602,"message":"invalid params"}` ✅ |

### 3.3 编码校验

源区 2 文件 + 开发版 1 文件（改动面）：**BOM=False、FFFD=0**（UTF-8 无 BOM 无替换符）。

### 3.4 红线遵守

- ✅ `--jobs 4` 全链路 / 分 crate（未全量 fmt）
- ✅ 未 kill 任何进程（验证用 server 进程验证后 Stop-Process 清理）
- ✅ 禁 force push（常规 push）
- ✅ 先读后改（W1 样板 abnormal.rs + W2-1 报告六处注册面 + taiji-knowledge-graph lib.rs/types.rs/embedding.rs 全读 + registry.rs 语义确认）
- ✅ 编码 UTF-8

---

## 四、注册顺序铁律确认

`rpc::kgraph::register_kgraph` 位于 `register_degraded`（lib.rs L50）之后、W2-1/2/3 注册之后追加——HashMap insert 后注册覆盖先注册，real 域不被降级 -32000 覆盖。`kgraph_query_unknown_id_invalid_params` 用例验证未知 id → -32602，反向证明 kgraph.query 已注册（注册后未走 -32601）。

---

## 五、并行协作记录（重要）

1. **deferred manifest 关键判定**：knowledge_graph 是 **Direct** 暴露（规划 §二 只读纯函数 → Direct），而 `get_deferred_tool_names()` 只返回 `default_exposure()==Deferred` 的工具（tool-contracts framework.rs L1585 实测）——初写把 knowledge_graph 误入 deferred manifest → 测试红 → 移除（content_render_kline 是 Deferred 留列，gbrain 也是 Deferred 留列）
2. **readonly manifest 判定**：knowledge_graph `is_readonly()=true` 进 readonly 清单（与 pattern/abnormal 只读工具同款）
3. **tool-provider-groups 单源真相**：feature 映射 + core.session tool_names + 展开断言三处同步加（`product_capability_provider_plan_covers_registry_manifest_in_order` 断言 provider plan == registry 顺序，缺一处即红）
4. **并行批次交错**：quant_tools.rs 被 W2-1/2/3 并行追加（文件增至 2975 行）；Cargo.toml base64 一度 duplicate（并行批中间态，后恢复）；kgraph.rs 编辑后文件被并行触碰触发 stale 重读——均按「先读后改」处理，零冲突
5. **taiji-server 进程验证期间反复退出**：非本批代码问题（启动日志正常 listening，验证 4+2 次调用全部成功），改用单次 ExecCommand 内联 启动→验证→清理 完成剩余用例

---

## 六、commit

- 源区 taiji-lvpa：`feat(W2-4): kgraph RPC 域 — kgraph.query/search/path/meta（taiji-knowledge-graph 策略图谱接线，纯只读静态数据）`
- 开发版 BitFun：`feat(W2-4): knowledge_graph 工具族 — knowledge_graph（9527 通道，Direct 只读，三层搜索）`

---

*LVPA Wave 2-4 knowledge_graph 批｜执行：姬码锋-执行官｜证据全部【实测】｜2026-08-13*
