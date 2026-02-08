**中文** | [English](README.md)

# BitFun E2E 测试

使用 WebDriverIO + tauri-driver 的 E2E 测试框架。

## 前置条件

### 1. 安装 tauri-driver

```bash
cargo install tauri-driver --locked
```

### 2. 构建应用

```bash
# 在项目根目录执行
npm run desktop:build
```

确保存在 `apps/desktop/target/release/BitFun.exe`（Windows）或 `apps/desktop/target/release/bitfun`（Linux）。

### 3. 安装 E2E 依赖

```bash
cd tests/e2e
npm install
```

## 运行测试

### 运行 L0 smoke 测试

```bash
cd tests/e2e
npm run test:l0
```

### 运行所有 smoke 测试

```bash
cd tests/e2e
npm run test:smoke
```

### 运行全部测试

```bash
cd tests/e2e
npm test
```

## 目录结构

```
tests/e2e/
├── config/                 # WebDriverIO 配置
│   ├── wdio.conf.ts       # 主配置
│   └── capabilities.ts    # 平台能力配置
├── specs/                  # 测试用例
│   ├── l0-smoke.spec.ts   # L0 smoke 测试
│   ├── startup/           # 启动相关测试
│   └── chat/              # 聊天相关测试
├── page-objects/           # Page Object 模型
├── helpers/                # 辅助工具
└── fixtures/               # 测试数据
```

## 故障排除

### 1. 找不到 tauri-driver

确保已安装 tauri-driver，并且 `~/.cargo/bin` 已加入 PATH：

```bash
cargo install tauri-driver --locked
```

### 2. 未构建应用

请先构建应用：

```bash
npm run desktop:build
```

### 3. 测试超时

Tauri 应用启动可能较慢；如有需要请在配置中调整超时时间。

## 添加测试

1. 在 `specs/` 下创建新的 `.spec.ts` 文件
2. 使用 Page Object 模式
3. 为被测 UI 元素添加 `data-testid` 属性

## data-testid 命名

格式：`{module}-{component}-{element}`

示例：
- `header-container` – 页头容器
- `chat-input-send-btn` – 聊天发送按钮
- `startup-open-folder-btn` – 启动页“打开文件夹”按钮

