[中文](README.zh-CN.md) | **English**

# BitFun E2E Tests

E2E test framework using WebDriverIO + tauri-driver.

## Prerequisites

### 1. Install tauri-driver

```bash
cargo install tauri-driver --locked
```

### 2. Build the app

```bash
# From project root
npm run desktop:build
```

Ensure `apps/desktop/target/release/BitFun.exe` (Windows) or `apps/desktop/target/release/bitfun` (Linux) exists.

### 3. Install E2E dependencies

```bash
cd tests/e2e
npm install
```

## Running tests

### Run L0 smoke tests

```bash
cd tests/e2e
npm run test:l0
```

### Run all smoke tests

```bash
cd tests/e2e
npm run test:smoke
```

### Run all tests

```bash
cd tests/e2e
npm test
```

## Directory structure

```
tests/e2e/
├── config/                 # WebDriverIO config
│   ├── wdio.conf.ts       # Main config
│   └── capabilities.ts    # Platform capabilities
├── specs/                  # Test specs
│   ├── l0-smoke.spec.ts   # L0 smoke tests
│   ├── startup/           # Startup-related tests
│   └── chat/              # Chat-related tests
├── page-objects/           # Page object model
├── helpers/                # Helper utilities
└── fixtures/               # Test data
```

## Troubleshooting

### 1. tauri-driver not found

Ensure tauri-driver is installed and `~/.cargo/bin` is in PATH:

```bash
cargo install tauri-driver --locked
```

### 2. App not built

Build the app:

```bash
npm run desktop:build
```

### 3. Test timeout

Tauri app startup can be slow; adjust timeouts in config if needed.

## Adding tests

1. Create a new `.spec.ts` file under `specs/`
2. Use the Page Object pattern
3. Add `data-testid` attributes to UI elements under test

## data-testid naming

Format: `{module}-{component}-{element}`

Examples:
- `header-container` – header container
- `chat-input-send-btn` – chat send button
- `startup-open-folder-btn` – startup open folder button
