# LVPA 基础设施层

> 对应 Phase 1 的 6 个基础设施 crate，位于 `software/taiji/src/crates/infra/`。

---

## 模块总览

| 模块 | Crate | 修仙名 | 职责 | 优先级 |
|:-----|:------|:-------|:-----|:-------|
| message-bus | `taiji-infra-message-bus` | 传音符 | 零语义字节传输层 | P0 |
| event-bus | `taiji-infra-event-bus` | 任务堂 | 语义事件总线 + KPI 调度 | P0 |
| transport | `taiji-infra-transport` | 传音阵 | Layer 3 连接层 IPC | P0 |
| db-store | `taiji-infra-db-store` | 灵脉 | SQLite 数据库存储 | P0 |
| config | `taiji-infra-config` | 天书阁 | 三平面配置管理 | P0 |
| monitor | `taiji-infra-monitor` | 天眼阵 | 监控与可观测性 | P0 |

### message-bus（传音符）

零语义字节传输层，所有跨模块通信的底层通道。

- **核心 trait**: `MessageBus` (publish / subscribe)
- **实现**: `InMemoryBus` (tokio broadcast + DashMap)
- **Layer 1 直通**: `RawMessage` + `RawMessageConsumer` 零拷贝路径

### event-bus（任务堂）

在 message-bus 之上构建的语义事件层，提供结构化事件分发和 KPI 评分调度。

- **核心类型**: `EventBus<M: MessageBus>` (泛型化), `TaijiEvent` (领域事件枚举)
- **KPI 调度**: `KpiScheduler` 按 success_rate / review_pass_rate / rework_rate / kpi_bonus 派单
- **L1 隔离**: L1 topic 注册表禁止异步事件推送

### transport（传音阵）

Layer 3 连接层组件，统一 IPC 传输抽象。

- **核心 trait**: `TransportAdapter` (send TransportMessage)
- **适配器**: `WsTransportAdapter` (WebSocket), `TauriTransportAdapter` (Tauri Emitter, feature gate)
- **不感知语义**: 仅传递序列化后的 event_name + payload

### db-store（灵脉）

结构化数据持久化，SQLite 单后端。

- **核心 trait**: `StorageBackend` (CRUD + 批量 + 分页 + 事务)
- **实现**: `SQLiteBackend` (sqlx-sqlite), WAL 模式默认
- **L1 缓冲**: `SharedBarBuffer` (arc_swap 无锁环缓冲)
- **迁移**: 内置版本化迁移框架 + checksum 校验
- **数据模型**: Agent / TaskEntity / RawBar / Freq / SymbolInfo

### config（天书阁）

三平面配置管理系统（env > file > defaults）。

- **核心 trait**: `ConfigManager` (load / get / set / reset / validate / subscribe)
- **变更广播**: `ConfigChangeEvent` 通过 tokio broadcast 发布
- **已知键校验**: `KNOWN_CONFIG_KEYS` 预注册 + validate 检查

### monitor（天眼阵）

全系统监控/日志/告警。

- **核心 trait**: `Monitor` (counter / gauge / histogram / health / alert / log)
- **指标分级**: P0-P4，内置 P0 核心指标集
- **告警引擎**: 阈值 / 变化率 / 超时检测 + 最小间隔去重
- **模块健康**: `ModuleHandle` (RAII 自动注册/注销)

---

## 依赖关系

```
Wave 0: 6 crate 骨架 (串行)
  └── 所有 crate 基础结构

Wave 1 (并行):
  ├── message-bus (无依赖)
  ├── transport  (无依赖)
  ├── db-store   (无依赖)
  └── config     (无依赖)

Wave 2 (串行, 依赖 Wave 1):
  ├── event-bus  ─→ 依赖 message-bus
  └── monitor    ─→ 依赖 config

Wave 3 (集成测试):
  ├── message-bus ↔ event-bus
  ├── event-bus ↔ transport
  ├── config ↔ monitor
  └── db-store ↔ SharedBarBuffer
```

---

## 快速开始

### 编译检查

```bash
cd software/taiji
cargo check --workspace
```

### 运行全部测试

```bash
cargo test --workspace
```

### 单 crate 测试

```bash
cargo test -p taiji-infra-message-bus
cargo test -p taiji-infra-event-bus
cargo test -p taiji-infra-transport
cargo test -p taiji-infra-db-store
cargo test -p taiji-infra-config
cargo test -p taiji-infra-monitor
```

### 生成文档

```bash
cargo doc --no-deps -p taiji-infra-message-bus -p taiji-infra-event-bus \
          -p taiji-infra-transport -p taiji-infra-db-store \
          -p taiji-infra-config -p taiji-infra-monitor
```

### clippy 检查

```bash
cargo clippy --workspace -- -D warnings
```

---

## 设计约束

1. **Layer 1 零阻塞**: message-bus/event-bus 不得向 L1 Compute 线程推送事件
2. **引用设计不抄代码**: 外部参考仅作为接口模式参考，非 Cargo 依赖
3. **无 unwrap/panic**: 所有错误通过 Result 返回
4. **STRICT 表**: db-store DDL 使用 SQLite STRICT + WITHOUT ROWID
5. **WAL 模式**: SQLite 默认 journal_mode = WAL
