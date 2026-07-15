<div align="center">

# Switchboard

**Edge device fleet control plane** — patch, watch, and command your IoT / edge fleet from one board.

![status](https://img.shields.io/badge/status-scaffolding-f7a331?style=for-the-badge)
![license](https://img.shields.io/badge/license-Apache--2.0-4a5568?style=for-the-badge)
![build](https://img.shields.io/badge/build-all--Rust-c9762f?style=for-the-badge)

</div>

Switchboard is a self-hostable control plane for fleets of IoT / edge devices:
onboard & provision, push configuration and OTA firmware, ingest telemetry, stream
and search logs, run rules & alerts, command devices remotely, and analyze it all.
**Simulation-first** — a built-in device simulator drives the whole platform live
with zero hardware, and a reference Rust agent shows how a real device plugs in.

Every device is a *line* on the board; the console is where you patch them.

## 🛠 Tech Stack

| Layer | Technology |
|-------|-----------|
| ![Rust](https://img.shields.io/badge/-Rust-000?logo=rust&logoColor=white) | Backend — Axum + Tokio (single self-hostable binary) |
| ![Leptos](https://img.shields.io/badge/-Leptos-ef3939?logo=leptos&logoColor=white) | Frontend — Leptos → WebAssembly (CSR SPA, built by Trunk) |
| ![MQTT](https://img.shields.io/badge/-MQTT-660066?logo=mqtt&logoColor=white) | Device ingestion (embedded broker) + HTTP + WebSocket |
| ![SQLite](https://img.shields.io/badge/-SQLite-003b57?logo=sqlite&logoColor=white) | Structural store + time-series rollups (WAL) |
| ![WebAssembly](https://img.shields.io/badge/-WASM-654ff0?logo=webassembly&logoColor=white) | Console runs in the browser as WebAssembly |

## Design

"Field Operations" — warm signal-amber / copper on graphite, with a switchboard
patch-bay motif. Two themes: dark *operations bunker* and light *field manual*.

## Layout

```
crates/
  core/   Axum backend (native) — API, WebSocket, MQTT ingest, serves the SPA
  web/    Leptos frontend (wasm32) — the console
  agent/  reference device agent          (later phase)
  sim/    device simulator                (later phase)
docs/
  TECHNICAL-BRIEF.md   full spec: modules, data model, phases
```

## Quickstart

```bash
# prerequisites: rustup, wasm32 target, trunk
rustup target add wasm32-unknown-unknown
cargo install trunk

# build the console
cd crates/web && trunk build && cd ../..

# run the control plane (serves API + the built console on :7930)
cargo run -p switchboard-core
```

Open http://localhost:7930 · MQTT on `:1883`.

## Status

Phase 0 — scaffold. See [docs/TECHNICAL-BRIEF.md](docs/TECHNICAL-BRIEF.md) for the
full module and phase plan.
