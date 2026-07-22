# taiji-magnet — Magnet Coordinate Positioning Engine

`ComputeNode` implementation: magnet positioning engine based on price-magnetism theory — support/resistance zone identification and attraction-force scoring.

> **Note:** This crate is closed-source and not published. Present only as a workspace placeholder.

## Architecture Position

```
taiji-engine (ComputeNode trait)
  └── taiji-magnet (MagnetNode)
```

## Core Concepts

| Concept | Description |
|---------|-------------|
| **Zone Detection** | Identifies magnetic price zones (support / resistance clusters) from historical bar data |
| **Magnetic Score** | Quantifies each zone's attraction strength based on touch count, volume, and recency |
| **Positioning Output** | Emits the nearest active magnet zone and its pull direction for the current bar |

## Dependencies

- `taiji-engine` — `ComputeNode` trait and `StateStore`
- `chrono` — timestamp handling
- `serde_json` — node configuration deserialization

## License

SPDX-License-Identifier: Apache-2.0 OR MIT
