# taiji-dvmi — Pivot Detection & Dual-Line Three-State Engine

`ComputeNode` implementation: DVMI trendline engine — pivot detection, dual-line construction, and three-state classification (up / down / neutral).

> **Note:** This crate is closed-source and not published. Present only as a workspace placeholder.

## Architecture Position

```
taiji-engine (ComputeNode trait)
  └── taiji-dvmi (DvmiNode)
```

## Core Concepts

| Concept | Description |
|---------|-------------|
| **Pivot Detection** | Identifies local extrema (swing highs / lows) from bar data |
| **Dual-Line Construction** | Two trendlines anchored at recent pivots — upper resistance, lower support |
| **Three-State Classification** | Each bar classified as `Up` (above upper line), `Down` (below lower line), or `Neutral` (between lines) |

## Dependencies

- `taiji-engine` — `ComputeNode` trait and `StateStore`
- `chrono` — timestamp handling
- `serde_json` — node configuration deserialization

## License

SPDX-License-Identifier: Apache-2.0 OR MIT
