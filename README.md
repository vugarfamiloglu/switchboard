<div align="center">

# Switchboard

**Edge device fleet control plane** — patch, watch, and command your IoT / edge fleet from one board.

![status](https://img.shields.io/badge/status-live-46c98a?style=for-the-badge)
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
| ![MQTT](https://img.shields.io/badge/-MQTT-660066?logo=mqtt&logoColor=white) | Device ingestion — an **embedded MQTT broker** (`:1883`) plus an HTTP ingest fallback |
| ![SQLite](https://img.shields.io/badge/-SQLite-003b57?logo=sqlite&logoColor=white) | Structural store (WAL) + in-memory live telemetry |
| ![WebAssembly](https://img.shields.io/badge/-WASM-654ff0?logo=webassembly&logoColor=white) | Console runs in the browser as WebAssembly |

## Features

- **Registry & provisioning** — devices, fleets, twins (desired/reported), claim-code enrollment
- **Ingestion** — devices publish to the embedded MQTT broker (`switchboard/{id}/telemetry`) or the HTTP ingest endpoint; both feed one pipeline that auto-registers unknown devices
- **Live telemetry** — per-device metrics streamed over WebSocket; the Overview *patch-bay wall* lights up per device
- **Fleet map** — devices plotted live by geographic position
- **Config & OTA** — reusable config profiles pushed to twins; firmware registry + canary rollout campaigns with live progress and one-click **rollback**
- **Remote commands** — reboot / ping / sync / identify, dispatched from the Commands page or per-device from its detail view, fulfilled with responses
- **Logs** — live-tail device log stream with level + text filters
- **Rules & alerts** — operator-authored rules (metric / operator / threshold / severity) the engine evaluates live to raise and auto-resolve alerts; enable, disable, or delete any rule; ack / resolve triage
- **Analytics** — live throughput chart, per-fleet availability, CSV export
- **Access control** — operator RBAC (owner / admin / operator / **viewer** = read-only), team management, session auth
- **Platform** — AES-256-GCM vault, SQLite backup, alert webhook, dark/light themes, ⌘-style live console

## Design

"Field Operations" — warm signal-amber / copper on graphite, with a switchboard
patch-bay motif. Two themes: dark *operations bunker* and light *field manual*.

## Layout

```
crates/
  core/   Axum backend (native) — API, WebSocket, ingest, simulator, serves the SPA
  web/    Leptos frontend (wasm32) — the console
  agent/  reference device agent — publishes telemetry to the control plane
docs/
  TECHNICAL-BRIEF.md   full spec: modules, data model, phases
```

## Quickstart

```bash
# prerequisites
rustup target add wasm32-unknown-unknown
cargo install trunk

# build the console, then run the control plane (serves API + console on :7930)
cd crates/web && trunk build && cd ../..
cargo run -p switchboard-core
```

Open **http://localhost:7930**. Sign in with the owner passcode `switchboard`, or as
an operator (e.g. `viewer@switchboard.local` / `switchboard` for read-only).

### Docker

```bash
docker build -t switchboard .
docker run -p 7930:7930 -v switchboard-data:/app/data switchboard
```

### Reference agent

Drive a real (or extra) device into the fleet. The agent is an MQTT client — it
connects to the control plane's **embedded broker** and publishes telemetry to
`switchboard/{device}/telemetry` every 2s; the broker forwards it into the ingest
pipeline, which auto-registers the device on first sight:

```bash
SWITCHBOARD_DEVICE=dev_field_01 SWITCHBOARD_NAME="Field Probe" \
  cargo run -p switchboard-agent
```

| Env | Default | Purpose |
|-----|---------|---------|
| `SWITCHBOARD_MQTT_HOST` | `localhost` | Broker host |
| `SWITCHBOARD_MQTT_PORT` | `1883` | Broker port |
| `SWITCHBOARD_DEVICE` | `dev_agent_demo` | Device id (topic + registry key) |
| `SWITCHBOARD_NAME` | `Field Agent Device` | Reported display name |

The same JSON payload shape is also accepted over HTTP at `POST /api/ingest/{device}`
for devices that can't speak MQTT.

## Configuration

| Env | Default | Purpose |
|-----|---------|---------|
| `SWITCHBOARD_PORT` | `7930` | Console / API / WebSocket port |
| `SWITCHBOARD_DATA` | `data` | SQLite + vault key directory |
| `SWITCHBOARD_PASSCODE` | `switchboard` | Owner sign-in passcode |

The embedded MQTT broker listens on `:1883` for device telemetry alongside the console/API port.

See [docs/TECHNICAL-BRIEF.md](docs/TECHNICAL-BRIEF.md) for the full module and data-model reference.
