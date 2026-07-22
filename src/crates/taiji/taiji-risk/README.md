# taiji-risk — Risk Control Rules & Parameter Management

`RiskMonitor` implementation: position sizing, ATR-based stop calculation, Kelly criterion, and configurable risk parameter management.

> **Note:** This crate is closed-source and not published. Present only as a workspace placeholder.

## Architecture Position

```
taiji-engine (RiskMonitor trait)
  └── taiji-risk (RiskManager)
         │
         ▼
taiji-executor (order execution)
```

## Core Concepts

| Concept | Description |
|---------|-------------|
| **Position Sizing** | Computes optimal position size from account equity, risk per trade, and stop distance |
| **ATR Stops** | Volatility-adjusted stop-loss levels based on Average True Range |
| **Kelly Criterion** | Applies fractional Kelly for bet sizing with configurable fraction parameter |
| **Risk Parameters** | Centralized risk config — max drawdown, daily loss limit, correlation cap, etc. |

## Dependencies

- `taiji-engine` — `RiskMonitor` trait and `StateStore`
- `taiji-executor` — order and position type definitions
- `chrono` — timestamp handling
- `serde_json` — risk parameter deserialization

## License

SPDX-License-Identifier: Apache-2.0 OR MIT
