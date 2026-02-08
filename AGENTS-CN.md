# AGENTS.md

## 项目概述

BitFun 是 AI 代理驱动的编程环境，使用 Rust 和 TypeScript 构建，采用多平台架构（桌面端/CLI/服务器）共享核心库。

### 架构

- **src/crates/events** - 事件定义（平台无关）
- **src/crates/core** - 核心业务逻辑（95%+ 代码复用）
  - `agentic/` - 代理系统（会话、工具、执行）
  - `service/` - 工作区、配置、文件系统、终端、Git
  - `infrastructure/` - AI 客户端、存储、日志、事件
- **src/crates/transport** - 传输适配器（CLI、Tauri、WebSocket）
- **src/crates/api-layer** - 平台无关处理器
- **src/apps/desktop** - Tauri 2.0 桌面应用
- **src/apps/cli** - 终端 UI（WIP）
- **src/apps/server** - Web 服务器（Axum + WebSocket）（WIP）
- **src/web-ui** - React 前端
  - `infrastructure/` - 主题、国际化、配置、状态管理、API 适配器
  - `component-library/` - 共享 UI 组件
  - `tools/` - 功能模块（编辑器、Git、终端、Mermaid...）
  - `flow_chat/` - 聊天界面
  - `locales/` - 翻译文件（en-US、zh-CN）

### 核心设计原则

1. **依赖注入** - 服务通过构造函数接收依赖
2. **EventEmitter 模式** - 使用 `Arc<dyn EventEmitter>` 而非 `AppHandle`
3. **TransportAdapter 模式** - 跨平台抽象通信
4. **平台无关核心** - Core 不包含平台特定依赖

### 技术栈

- **后端**: Rust 2021, Tokio, Tauri 2.0, Axum
- **前端**: React 18, TypeScript, Vite, Zustand

## 开发命令

```bash
# 桌面端
npm run desktop:dev              # 开发模式

# E2E
npm run e2e:test
```

## 关键规则

### 日志规范

**规则：** 仅英文、禁止 emoji、结构化数据、避免冗余日志

- **前端**: `src/web-ui/LOGGING.md` - 使用 `createLogger('ModuleName')`
- **后端**: `src/crates/LOGGING.md` - 使用 `log::{info, debug, ...}` 宏

### 传输层

**核心代码中禁止使用平台特定 API：**
- ❌ `use tauri::AppHandle`
- ✅ `use bitfun_events::EventEmitter`

### Tauri 命令

**命名规范：** 命令 `snake_case`，Rust `snake_case`，TypeScript `camelCase`

**始终使用结构化请求格式：**

```rust
#[tauri::command]
pub async fn your_command(
    state: State<'_, AppState>,
    request: YourRequest,
) -> Result<YourResponse, String>
```

```typescript
await api.invoke('your_command', { request: { ... } });
```

### 前端复用

开发前端功能时，复用现有基础设施：
- **主题**: `infrastructure/theme/` - useTheme, useThemeToggle
- **国际化**: `infrastructure/i18n/` + `locales/` - useI18n, t()
- **组件**: `component-library/` - 共享 UI 组件
- **状态**: 各模块内的 Zustand Store

## 核心组件

### 代理系统

```
SessionManager → Session → DialogTurn → ModelRound
```

- `ConversationCoordinator` - 协调轮次
- `ExecutionEngine` - 多轮循环
- `ToolPipeline` - 工具并发执行

### 会话持久化

位置：`.bitfun/sessions/{session_id}/`

### 工具开发

在 `agentic/tools/registry.rs` 注册：
1. 实现 `Tool` trait
2. 定义输入/输出类型
3. 处理流式传输（如适用）

### 添加代理

在 `agentic/agents/`：
1. 创建代理文件
2. 在 `prompts/` 定义提示词
3. 在 `registry.rs` 注册
