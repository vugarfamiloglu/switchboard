# Switchboard — Technical Brief

> **Switchboard** — an edge device **fleet control plane**. Every device is a
> "line" on the board; the console is where you patch, provision, configure,
> update, observe, command, and analyze whole fleets of IoT / edge devices.
> Simulation-first: a built-in device simulator makes the entire platform
> demoable without a single piece of physical hardware, while a reference device
> agent shows how a real device plugs in.

Status: **planning / pre-build**. Target build date: **2026-07-15** (next session).

**Locked decisions** (confirmed): Name **Switchboard** · Stack **all-Rust
(Axum backend + Leptos/WASM frontend)** · Design **"Field Operations"**.

---

## 1. Name & metaphor

**Switchboard** — the operator's patch-bay: rows of jacks, amber indicator lamps,
patch cables routing many lines through one board. As an IoT control plane the
metaphor is exact — each device is a *line*, the console is the *board* where an
operator patches, monitors, and routes them. Ownable, no collision with prior
projects, and it hands the design a ready-made visual identity (jack-field,
indicator lamps, patch cables).

Tagline: **"Patch, watch, and command your device fleet."**

---

## 2. Stack — all-Rust (locked)

Deliberately rotated away from Cipherlane (Go + React) and every recent build.
IoT ingestion is latency/throughput-bound and the console is telemetry-heavy →
**one language, end to end, in Rust.**

| Layer | Choice | Notes |
|-------|--------|-------|
| **Backend** | **Rust — Axum + Tokio** | async ingestion, single self-hostable binary that also serves the frontend bundle |
| **Frontend** | **Leptos (Rust → WASM), CSR SPA via Trunk** | signals reactivity (`create_signal`/`create_resource`/`create_effect`), `view!` macro; Axum serves the `trunk build` output + `/api` + `/ws`. (SSR via `cargo-leptos` is the fallback if we want it.) |
| **Ingestion** | **MQTT** (embedded broker `rumqttd`) + HTTP + WebSocket (+ optional CoAP) | MQTT is the IoT standard; WS bridges telemetry to the browser |
| **Structural store** | **SQLite** (`rusqlite`/`sqlx`, WAL, single conn) | devices, twins, config, rules, users, campaigns; single file under `data/` |
| **Time-series** | SQLite raw table + background **rollups** (1m/5m/1h) + retention prune; **in-memory live state** (`DashMap`) | mirrors Cipherlane's in-memory-live + SQLite-structural split (proven, liked) |
| **Live UI transport** | WebSocket via `web-sys`/`gloo-net` in Leptos | telemetry, log tail, status, OTA progress, alerts |
| **HTTP client (WASM)** | `gloo-net` / `reqwasm` to `/api` | consistent `{ok,data,error}` envelope |
| **Charts** | **hand-rolled SVG inside `view!`** | proven in Cipherlane; reactive SVG nodes, no heavy dep |
| **Map** | **MapLibre GL via `wasm-bindgen` JS interop** (embed the JS lib, drive from Rust) — or a custom SVG/canvas map | the one place we cross into JS; isolate behind a small module |
| **Styling** | vanilla CSS design tokens (global sheet via Trunk) | per house style |
| **Fan-out (optional)** | Redis pub/sub | multi-instance WS fan-out; optional |
| **Packaging** | multi-stage Docker; Axum serves the WASM bundle | one artifact, self-hostable |

**Device side:** a reference **Rust device agent** binary + a **device simulator**
(spawns N virtual devices: telemetry + logs + command/OTA responses).

**Ports:** console/API/WS **`:7930`** · MQTT **`:1883`** · MQTT-TLS **`:8883`** (optional).

**Known considerations for the all-Rust path:** WASM bundle size (keep deps lean,
`opt-level="z"` + `wasm-opt`); charting is hand-built SVG (fine — on-brand); the map
needs `wasm-bindgen` interop with MapLibre (the only JS boundary — wrap it small).

---

## 3. Design direction — "Field Operations" (locked)

House app-shell stays (**sidebar 248px + sticky topbar + workbench**); visual language is new.

Warm **signal-amber / copper** primary on **graphite/charcoal**, with **live-green**
(healthy) and **alarm-red** (fault) as semantic states. The **switchboard motif**
drives it: **amber indicator lamps**, a **jack-field grid**, **patch-cable connectors**,
VU-meter-style telemetry readouts. Instrumentation feel without repeating Cipherlane's
cool blueprint grid.

- **Type (new pairing):** **Archivo** (display, industrial grotesque) + **JetBrains Mono**
  (data/labels) — distinct from Cipherlane's Space Grotesk + IBM Plex Mono.
- **Two first-class themes:** dark **"operations bunker"** + light **"field manual /
  engineering worksheet."**
- **Signature elements** (Switchboard's equivalent of Cipherlane's live topology):
  1. **Patch-bay wall** — the device grid as a jack-field; each device is a jack with a
     live amber status lamp that pulses with telemetry; patch cables link related devices.
  2. **Live fleet map** — geo markers that pulse with device status.
  3. **VU/telemetry ticker** — streaming instrument readout of the live stream.
- Motion: compositor-only (transform/opacity), `prefers-reduced-motion` respected.

---

## 4. System architecture

```
 Devices / Simulator                Switchboard core (Rust/Axum)          Console (Leptos/WASM)
 ┌───────────────┐   MQTT 1883   ┌──────────────────────────┐   WS 7930   ┌────────────────┐
 │ agent / sim   │──telemetry──▶ │ broker → ingest pipeline  │──live────▶ │ patch-bay wall │
 │  · telemetry  │──logs──────▶  │  · auth (cert/psk/claim)  │            │ map · device   │
 │  · logs       │◀─desired────  │  · twin reconcile         │  REST 7930 │ config · OTA   │
 │  · twin       │◀─cmd/ota────  │  · rules/alerts engine    │◀─mgmt────▶ │ logs · rules   │
 │  · cmd/ota    │──cmd res────▶ │  · in-mem live (DashMap)  │            │ analytics      │
 └───────────────┘               │  · SQLite + rollups       │            └────────────────┘
                                 │  · vault · RBAC · audit   │
                                 └──────────────────────────┘
```

**Data flow:** device → MQTT topic → ingest (auth, validate, route) → update
in-memory live state + append to SQLite → WS broadcast to subscribed UI clients →
rules engine evaluates windows → alerts/automations fire → webhooks/commands out.

---

## 5. Modules (functional requirements)

### M1 — Device Registry & Provisioning
ULID device id, enrollment via **claim code / X.509 device cert / pre-shared key**,
device **twin** (desired vs reported, versioned), model & firmware version, tags,
**fleets/groups**, lifecycle (`provisioned → active → quarantined → retired`), bulk
import, search/filter, per-device profile page.

### M2 — Connectivity / Ingestion Gateway
Embedded **MQTT broker** (per-device auth, topic ACL), **HTTP** + **WebSocket** ingest
bridges, **Last-Will/keepalive** → online/offline, connection log, routing, backpressure-safe.

### M3 — Telemetry & Time-Series
Typed metric ingestion (number/bool/geo/string), **live latest** in memory, **rollups**
(1m/5m/1h), retention, per-device & per-fleet aggregates, live streaming, unit metadata.

### M4 — Configuration & Firmware (OTA)
**Config profiles** (schema + values), **desired-state push** via twin, **firmware registry**
(version, size, SHA-256, model targeting), **OTA campaigns** with **canary rollout (%)**,
live per-device progress, pause/resume, **rollback**.

### M5 — Remote Commands & Device RPC
Commands (reboot, run action, set param), **request/response** with correlation ids, a
**remote RPC/shell** panel, timeouts, full **command audit**.

### M6 — Log Streaming & Search
Structured log ingestion (level, ts, msg, fields), **live tail**, filter/search by
device/level/text/time, severity coloring, saved queries, retention.

### M7 — Rules, Alerts & Automations
**Rule engine** over telemetry windows (`temp > 80 for 5m`), **alerts** (open/ack/resolve,
severity), routing (**webhook**, email-sim), **automations** (event → command/tag/notify),
**maintenance windows** that suppress alerts.

### M8 — Analytics & Dashboards
Fleet-health overview, per-metric charts with anomaly bands, **uptime/SLA**, **geo map**,
custom tiles, **CSV + PNG export** (reuse Cipherlane's SVG→PNG + CSV helpers).

### M9 — Access Control (Orgs, RBAC, Keys)
**Orgs/projects**, roles **Owner / Admin / Operator / Viewer** (viewer = read-only, reusing
Cipherlane's proven write-guard pattern), **API keys** (programmatic + device), **audit log**.

### M10 — Device Simulator & Agent
**Simulator**: N virtual devices across models (thermostat, gateway, tracker, meter…),
realistic telemetry (mean-reverting walk), occasional faults/anomalies (to exercise
rules), logs, twin reports, command/OTA responses — drives the whole UI live with zero
hardware. **Reference Rust agent** (`cmd/agent`): MQTT connect, publish telemetry/logs,
apply desired state, execute commands, OTA download+verify; cross-compiles Linux/ARM;
`--dry-run` on dev machines.

### M11 — Platform / Settings
**AES-256-GCM vault** for secrets, **backup/restore** (SQLite snapshot), webhooks,
retention config, theme switch, **in-app log monitor**, **⌘K command palette**.

---

## 6. Data model (entities)

`Org` · `User(role)` · `ApiKey` · `Device(id, name, model, fw_version, status, last_seen,
fleet_id, org_id, auth_type, credential)` · `Fleet/Group(tags)` · `DeviceTwin(desired,
reported, version)` · `TelemetryPoint(device, metric, ts, value)` · `TelemetryRollup(metric,
bucket, min/max/avg/count)` · `LogEntry(level, ts, msg, fields)` · `ConfigProfile(schema,
values, targets)` · `FirmwareArtifact(version, size, sha256, model)` · `OtaCampaign(target,
strategy/canary%, status, per-device progress)` · `Rule(expr, window, severity, actions)` ·
`Alert(rule, device, state)` · `Command(device, name, args, status, result)` · `Webhook` ·
`AuditEvent` · `VaultSecret`. All: ULID id, `created_at`/`updated_at`, indexed for the
query patterns above.

---

## 7. API & protocol surface

**REST (`:7930/api`)** — `/auth`, `/devices` (+ `/enroll`, `/{id}/twin`, `/{id}/command`),
`/fleets`, `/telemetry`, `/logs`, `/config-profiles`, `/firmware`, `/ota`, `/rules`,
`/alerts`, `/automations`, `/analytics`, `/orgs`, `/users`, `/api-keys`, `/webhooks`,
`/audit`, `/vault`, `/backup`, `/restore`. Envelope `{ok,data,error}`.

**WebSocket** — live telemetry, log tail, device status, OTA progress, alert stream.

**MQTT topics** — `switchboard/{org}/{device}/telemetry` · `/logs` · `/state/reported`
(device→cloud) · `/state/desired` · `/cmd/req` + `/cmd/res` · `/ota/offer` + `/ota/progress`
· `$connected` (LWT). Per-device topic ACL.

---

## 8. Security model

Device auth: per-device **X.509 cert** or **PSK/claim-code**; topic ACL per device.
Operator auth: passcode + operator RBAC (owner/admin/operator/**viewer read-only**, enforced
by a write-guard middleware — proven in Cipherlane). **API keys** for programmatic access.
**AES-256-GCM vault** for secrets. Rate limiting on ingest + public endpoints. Parameterized
SQL only. Audit log on every mutation. HTTPS/HSTS + security headers in prod.

---

## 9. Build phases

- **Phase 0 — Scaffold.** Cargo workspace (`core` Axum + `web` Leptos + `agent` + `sim`),
  Apache-2.0, git identity, ~20 topics. House app-shell (sidebar + sticky topbar + workbench)
  in Leptos, dark/light theming, passcode + operator RBAC, vault, SQLite bootstrap, embedded
  MQTT broker, WS hub, simulator skeleton. Trunk build served by Axum. Port 7930.
- **Phase 1 — Registry + Ingestion + Live.** Device CRUD/enroll, fleets/tags, twin, MQTT
  telemetry+log ingest, in-memory live + WS broadcast, Overview + **patch-bay wall** + device
  detail (live gauges/sparklines), simulator emitting realistic data.
- **Phase 2 — Config + OTA + Commands.** Config profiles + desired-state push, firmware
  registry + OTA campaigns (canary + progress + rollback), remote commands + RPC panel + audit.
- **Phase 3 — Logs + Rules + Alerts.** Log search / live-tail / saved queries, rule engine over
  telemetry windows, alerts (open/ack/resolve) + webhook routing, automations, maintenance windows.
- **Phase 4 — Analytics + Map + Multi-tenant.** Fleet map (MapLibre interop, pulsing markers),
  analytics dashboards (charts, anomaly bands, uptime/SLA), CSV/PNG export, orgs/projects + full
  RBAC + API keys + audit log.
- **Phase 5 — Polish + Tests + Docs + Deploy.** Rust unit/integration tests + Leptos component
  tests + Playwright E2E, Docker multi-stage, README (shields tech-stack table), in-app log
  monitor, resizeable tables, ⌘K palette, WASM size + performance pass, browser-verified.

Each phase → granular conventional commits (module-per-feat) + push (public, no AI attribution).
Verify in-browser via chrome-devtools MCP (preview tool broken on Windows).

---

## 10. Conventions honored · non-goals

**Honored:** house app-shell layout, ConfirmModal/PromptModal (no `window.confirm/prompt`),
AES vault, Toaster pubsub, resizeable DataTable, in-app log monitor, collapsible sidebar,
sticky topbar with a live signature badge, password eye-toggle, dark+light, ⌘K palette,
RBAC read-only role, CSV/PNG export helpers, detailed README with shields tech-stack table.
**i18n:** single-language (English UI), same call as Cipherlane Phase C.

**Non-goals:** not a real firmware *build* system; simulation-first (no kernel/hardware
required, but a real agent is provided); not a full Grafana/Prometheus replacement —
opinionated fleet control, not a generic observability suite.
