# taiji-thrust — Triple-Push Thrust Detection Engine

`ComputeNode` implementation: three-push exhaustion pattern detection — counts consecutive directional thrusts and signals exhaustion when the third push completes.

> **Note:** This crate is closed-source and not published. Present only as a workspace placeholder.

## Architecture Position

```
taiji-engine (ComputeNode trait)
  └── taiji-thrust (ThrustNode)
```

## Core Concepts

| Concept | Description |
|---------|-------------|
| **Push Counting** | Tracks consecutive directional bars (higher-high / lower-low sequences) |
| **Exhaustion Signal** | Fires when a third consecutive push completes — classic three-push exhaustion pattern |
| **Impulse vs Correction** | Distinguishes impulsive thrusts from corrective retracements using bar magnitude comparison |

## Dependencies

- `taiji-engine` — `ComputeNode` trait and `StateStore`
- `chrono` — timestamp handling
- `serde_json` — node configuration deserialization

## License

SPDX-License-Identifier: Apache-2.0 OR MIT
