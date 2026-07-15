//! Switchboard console — routed, authenticated, live. Overview's patch-bay wall
//! and the topbar badge are driven by the WebSocket telemetry stream.

use leptos::prelude::*;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::hooks::{use_location, use_navigate, use_params_map};
use leptos_router::path;
use wasm_bindgen_futures::spawn_local;

use crate::api::{self, Device, DeviceLive};
use crate::live::{provide_live, use_live};

#[derive(Clone, Copy)]
struct Auth {
    authed: RwSignal<Option<bool>>,
    role: RwSignal<String>,
    name: RwSignal<String>,
}

fn use_auth() -> Auth {
    use_context::<Auth>().expect("Auth provided at app root")
}

#[derive(Clone, Copy)]
struct ThemeCtx {
    theme: ReadSignal<String>,
    set: WriteSignal<String>,
}

#[component]
pub fn App() -> impl IntoView {
    let auth = Auth {
        authed: RwSignal::new(None),
        role: RwSignal::new(String::new()),
        name: RwSignal::new(String::new()),
    };
    provide_context(auth);
    provide_live();

    spawn_local(async move {
        match api::me().await {
            Ok(s) if s.authenticated => {
                auth.role.set(s.role);
                auth.name.set(s.name);
                auth.authed.set(Some(true));
            }
            _ => auth.authed.set(Some(false)),
        }
    });

    let (theme, set_theme) = signal(String::from("dark"));
    provide_context(ThemeCtx { theme, set: set_theme });

    view! {
        <div class=move || format!("app-root theme-{}", theme.get())>
            {move || match auth.authed.get() {
                None => view! { <div class="boot mono">"Connecting to control plane…"</div> }.into_any(),
                Some(false) => view! { <Login/> }.into_any(),
                Some(true) => view! { <Shell theme set_theme/> }.into_any(),
            }}
        </div>
    }
}

#[component]
fn Login() -> impl IntoView {
    let auth = use_auth();
    let (mode, set_mode) = signal(String::from("owner"));
    let (passcode, set_passcode) = signal(String::new());
    let (email, set_email) = signal(String::new());
    let (password, set_password) = signal(String::new());
    let (busy, set_busy) = signal(false);
    let (error, set_error) = signal(String::new());

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        set_busy.set(true);
        set_error.set(String::new());
        let creds = if mode.get() == "operator" {
            serde_json::json!({ "email": email.get(), "password": password.get() })
        } else {
            serde_json::json!({ "passcode": passcode.get() })
        };
        spawn_local(async move {
            match api::login(&creds).await {
                Ok(s) if s.authenticated => {
                    auth.role.set(s.role);
                    auth.name.set(s.name);
                    auth.authed.set(Some(true));
                }
                Ok(_) => {
                    set_error.set("Sign-in failed".into());
                    set_busy.set(false);
                }
                Err(e) => {
                    set_error.set(e);
                    set_busy.set(false);
                }
            }
        });
    };

    view! {
        <div class="login">
            <div class="login-card">
                <div class="login-brand"><span class="brand-mark"><Jack/></span><span class="brand-word">"Switch"<b>"board"</b></span></div>
                <h1 class="login-title">{move || if mode.get() == "operator" { "Operator sign-in" } else { "Control plane access" }}</h1>
                <p class="login-sub">"Patch, watch, and command your device fleet."</p>
                <form on:submit=submit class="login-form">
                    {move || if mode.get() == "operator" {
                        view! {
                            <label class="field"><span class="field-label mono">"EMAIL"</span>
                                <input class="input mono" prop:value=move || email.get() on:input=move |e| set_email.set(event_target_value(&e)) placeholder="viewer@switchboard.local"/></label>
                            <label class="field"><span class="field-label mono">"PASSWORD"</span>
                                <input class="input mono" type="password" prop:value=move || password.get() on:input=move |e| set_password.set(event_target_value(&e))/></label>
                        }.into_any()
                    } else {
                        view! {
                            <label class="field"><span class="field-label mono">"OPERATOR PASSCODE"</span>
                                <input class="input mono" type="password" prop:value=move || passcode.get() on:input=move |e| set_passcode.set(event_target_value(&e)) placeholder="switchboard"/></label>
                        }.into_any()
                    }}
                    {move || (!error.get().is_empty()).then(|| view! { <div class="login-error mono">{error.get()}</div> })}
                    <button class="btn btn-primary" type="submit" disabled=move || busy.get()>{move || if busy.get() { "Verifying…" } else { "Sign in" }}</button>
                </form>
                <button type="button" class="login-switch" on:click=move |_| set_mode.update(|m| *m = if m == "owner" { "operator".into() } else { "owner".into() })>
                    {move || if mode.get() == "owner" { "Sign in as an operator →" } else { "← Owner passcode sign-in" }}
                </button>
                <div class="login-foot mono">{move || if mode.get() == "operator" { "e.g. viewer@… · switchboard" } else { "default · switchboard" }}</div>
            </div>
        </div>
    }
}

#[component]
fn Shell(theme: ReadSignal<String>, set_theme: WriteSignal<String>) -> impl IntoView {
    view! {
        <Router>
            <div class="app-shell">
                <Sidebar/>
                <TopBar theme set_theme/>
                <main class="workbench">
                    <Routes fallback=|| view! { <div class="empty">"Not found."</div> }>
                        <Route path=path!("/") view=Overview/>
                        <Route path=path!("/devices") view=Devices/>
                        <Route path=path!("/devices/:id") view=DeviceDetail/>
                        <Route path=path!("/alerts") view=Alerts/>
                        <Route path=path!("/logs") view=Logs/>
                        <Route path=path!("/commands") view=Commands/>
                        <Route path=path!("/config") view=Config/>
                        <Route path=path!("/firmware") view=Firmware/>
                        <Route path=path!("/team") view=Team/>
                        <Route path=path!("/settings") view=Settings/>
                        <Route path=path!("/analytics") view=Analytics/>
                        <Route path=path!("/fleets") view=Fleets/>
                        <Route path=path!("/map") view=FleetMap/>
                        <Route path=path!("/rules") view=Rules/>
                    </Routes>
                </main>
            </div>
        </Router>
    }
}

const NAV: &[(&str, &[(&str, &str, &str)])] = &[
    ("Operations", &[("Overview", "OV", "/"), ("Fleet Map", "MP", "/map")]),
    ("Fleet", &[("Devices", "DV", "/devices"), ("Fleets", "FL", "/fleets")]),
    ("Delivery", &[("Config", "CF", "/config"), ("Firmware", "FW", "/firmware"), ("Commands", "CM", "/commands")]),
    ("Observe", &[("Logs", "LG", "/logs"), ("Rules", "RL", "/rules"), ("Alerts", "AL", "/alerts")]),
    ("Insights", &[("Analytics", "AN", "/analytics")]),
    ("Admin", &[("Team", "TM", "/team"), ("Settings", "ST", "/settings")]),
];

#[component]
fn Sidebar() -> impl IntoView {
    let path_sig = use_location().pathname;
    view! {
        <aside class="sidebar">
            <div class="brand"><span class="brand-mark"><Jack/></span><span class="brand-word">"Switch"<b>"board"</b></span></div>
            <nav class="nav">
                {NAV.iter().map(|(group, items)| view! {
                    <div class="nav-group">
                        <div class="nav-group-label mono">{*group}</div>
                        {items.iter().map(|(label, glyph, href)| {
                            if *href == "#" {
                                view! { <span class="nav-link is-soon"><span class="nav-glyph mono">{*glyph}</span><span class="nav-label">{*label}</span><span class="soon mono">"soon"</span></span> }.into_any()
                            } else {
                                let href = *href;
                                view! {
                                    <a class=move || if path_sig.get() == href { "nav-link is-active" } else { "nav-link" } href=href>
                                        <span class="nav-glyph mono">{*glyph}</span><span class="nav-label">{*label}</span>
                                    </a>
                                }.into_any()
                            }
                        }).collect_view()}
                    </div>
                }).collect_view()}
            </nav>
            <div class="sidebar-foot mono">"patch · watch · command"</div>
        </aside>
    }
}

#[component]
fn TopBar(theme: ReadSignal<String>, set_theme: WriteSignal<String>) -> impl IntoView {
    let auth = use_auth();
    let live = use_live();
    let agg = move || live.telemetry.get().aggregate;
    let logout = move |_| {
        spawn_local(async move {
            let _ = api::logout().await;
            auth.authed.set(Some(false));
        });
    };
    view! {
        <header class="topbar">
            <div class="tb-left">
                <span class="tb-mark"><Jack/></span>
                <span class="tb-crumb mono">"CONTROL"</span>
                <span class="tb-div"></span>
                <div class="tb-titles">
                    <span class="tb-title">"Switchboard"</span>
                    <span class="tb-sub">{move || format!("{} of {} devices online", agg().online, agg().total)}</span>
                </div>
                {move || (auth.role.get() == "viewer").then(|| view! { <span class="ro-chip mono">"READ-ONLY"</span> })}
            </div>
            <div class="tb-badge">
                <span class=move || if live.connected.get() { "tb-badge-dot is-live" } else { "tb-badge-dot" }></span>
                <span class="tb-badge-val mono">{move || format!("{:.0}", agg().msg_rate)}</span>
                <span class="tb-badge-unit mono">"msg/s"</span>
                <span class="tb-badge-sep"></span>
                <span class="mono u-muted">{move || format!("{} alerts", agg().alerts)}</span>
            </div>
            <div class="tb-right">
                <button class="tb-icon-btn" title="Toggle theme" on:click=move |_| set_theme.update(|t| *t = if t == "dark" { "light".into() } else { "dark".into() })>
                    {move || if theme.get() == "dark" { "☀" } else { "☾" }}
                </button>
                <button class="tb-icon-btn" title="Sign out" on:click=logout>"⇥"</button>
            </div>
        </header>
    }
}

fn lamp_tone(d: &Device, dl: &DeviceLive) -> &'static str {
    if !dl.online {
        return "down";
    }
    if dl.metrics.get("tempC").is_some_and(|t| *t > 30.0) {
        return "alarm";
    }
    if dl.metrics.get("batteryPct").is_some_and(|b| *b < 15.0) {
        return "alarm";
    }
    if d.status == "quarantined" {
        return "warn";
    }
    "up"
}

#[component]
fn Overview() -> impl IntoView {
    let live = use_live();
    let devices = RwSignal::new(Vec::<Device>::new());
    spawn_local(async move {
        if let Ok(d) = api::devices().await {
            devices.set(d);
        }
    });
    let navigate = use_navigate();
    let agg = move || live.telemetry.get().aggregate;
    let uptime = move || {
        let a = agg();
        if a.total == 0 { 100.0 } else { a.online as f64 / a.total as f64 * 100.0 }
    };

    view! {
        <div class="page-head"><div>
            <h1 class="page-title">"Control Overview"</h1>
            <p class="page-desc">"Live posture across every device, fleet, and edge line."</p>
        </div></div>

        <div class="kpi-grid">
            <div class="stat stat-up"><div class="stat-label mono">"DEVICES ONLINE"</div><div class="stat-value">{move || agg().online}<span class="stat-unit mono">{move || format!(" / {}", agg().total)}</span></div></div>
            <div class="stat stat-amber"><div class="stat-label mono">"TELEMETRY RATE"</div><div class="stat-value">{move || format!("{:.0}", agg().msg_rate)}<span class="stat-unit mono">" msg/s"</span></div></div>
            <div class=move || format!("stat stat-{}", if agg().alerts > 0 { "alarm" } else { "up" })><div class="stat-label mono">"OPEN ALERTS"</div><div class="stat-value">{move || agg().alerts}<span class="stat-unit mono">" faults"</span></div></div>
            <div class="stat stat-up"><div class="stat-label mono">"FLEET UPTIME"</div><div class="stat-value">{move || format!("{:.1}", uptime())}<span class="stat-unit mono">" %"</span></div></div>
        </div>

        <section class="panel">
            <div class="panel-head"><div class="panel-title">"Patch bay"</div><div class="panel-note mono">{move || format!("{} lines · live", devices.get().len())}</div></div>
            <div class="patchbay">
                {move || {
                    let tele = live.telemetry.get();
                    let navigate = navigate.clone();
                    devices.get().into_iter().map(|d| {
                        let dl = tele.devices.get(&d.id).cloned().unwrap_or_default();
                        let tone = lamp_tone(&d, &dl);
                        let id = d.id.clone();
                        let navigate = navigate.clone();
                        view! {
                            <button class=format!("jackcell jack-{}", tone) title=d.name.clone() on:click=move |_| navigate(&format!("/devices/{}", id), Default::default())>
                                <span class=format!("lamp lamp-{}", tone)></span>
                                <span class="jack-name">{d.name.clone()}</span>
                            </button>
                        }
                    }).collect_view()
                }}
            </div>
        </section>
    }
}

#[component]
fn Devices() -> impl IntoView {
    let live = use_live();
    let devices = RwSignal::new(Vec::<Device>::new());
    spawn_local(async move {
        if let Ok(d) = api::devices().await {
            devices.set(d);
        }
    });
    let (q, set_q) = signal(String::new());
    let navigate = use_navigate();

    view! {
        <div class="page-head"><div>
            <h1 class="page-title">"Devices"</h1>
            <p class="page-desc">"Every enrolled device and its live status."</p>
        </div></div>
        <div class="toolbar">
            <input class="search mono" placeholder="Search devices…" prop:value=move || q.get() on:input=move |e| set_q.set(event_target_value(&e))/>
            <span class="toolbar-count mono">{move || format!("{} devices", devices.get().len())}</span>
        </div>
        <div class="dtable">
            <div class="dt-head mono">
                <span>"STATUS"</span><span>"DEVICE"</span><span>"MODEL"</span><span>"FLEET"</span><span>"FW"</span>
            </div>
            {move || {
                let tele = live.telemetry.get();
                let needle = q.get().to_lowercase();
                let navigate = navigate.clone();
                devices.get().into_iter()
                    .filter(|d| needle.is_empty() || format!("{} {} {} {}", d.name, d.model, d.fleet_name.clone().unwrap_or_default(), d.tags).to_lowercase().contains(&needle))
                    .map(|d| {
                        let online = tele.devices.get(&d.id).map(|l| l.online).unwrap_or(false);
                        let id = d.id.clone();
                        let navigate = navigate.clone();
                        view! {
                            <div class="dt-row" on:click=move |_| navigate(&format!("/devices/{}", id), Default::default())>
                                <span><span class=if online { "dot dot-up" } else { "dot dot-down" }></span>{if online { " online" } else { " offline" }}</span>
                                <span class="cell-strong">{d.name.clone()}</span>
                                <span class="mono u-muted">{d.model.clone()}</span>
                                <span>{d.fleet_name.clone().unwrap_or_default()}</span>
                                <span class="mono u-muted">{d.fw_version.clone()}</span>
                            </div>
                        }
                    }).collect_view()
            }}
        </div>
    }
}

#[component]
fn DeviceDetail() -> impl IntoView {
    let params = use_params_map();
    let live = use_live();
    let detail = RwSignal::new(None::<api::DeviceDetail>);

    Effect::new(move |_| {
        let id = params.read().get("id").unwrap_or_default();
        if id.is_empty() {
            return;
        }
        spawn_local(async move {
            if let Ok(d) = api::device(&id).await {
                detail.set(Some(d));
            }
        });
    });

    view! {
        {move || match detail.get() {
            None => view! { <div class="empty">"Loading device…"</div> }.into_any(),
            Some(d) => {
                let id = d.device.id.clone();
                let metrics = move || live.telemetry.get().devices.get(&id).cloned().unwrap_or_default();
                let m_pill = metrics.clone();
                let m_text = metrics.clone();
                view! {
                    <div class="page-head"><div>
                        <a class="back-link mono" href="/devices">"← Devices"</a>
                        <h1 class="page-title">{d.device.name.clone()}</h1>
                        <p class="page-desc mono">{format!("{} · fw {}", d.device.model, d.device.fw_version)}</p>
                    </div></div>
                    <div class="detail-grid">
                        <section class="panel">
                            <div class="panel-head"><div class="panel-title">"Live metrics"</div><div class=move || if m_pill().online { "pill pill-up mono" } else { "pill pill-down mono" }>{move || if m_text().online { "online" } else { "offline" }}</div></div>
                            <div class="metric-grid">
                                {move || {
                                    let mut ms: Vec<(String, f64)> = metrics().metrics.into_iter().collect();
                                    ms.sort_by(|a, b| a.0.cmp(&b.0));
                                    ms.into_iter().map(|(k, v)| view! {
                                        <div class="metric"><div class="metric-k mono">{k}</div><div class="metric-v">{format!("{:.1}", v)}</div></div>
                                    }).collect_view()
                                }}
                            </div>
                        </section>
                        <section class="panel">
                            <div class="panel-head"><div class="panel-title">"Registry"</div></div>
                            <dl class="kv">
                                <div><dt class="mono">"STATUS"</dt><dd>{d.device.status.clone()}</dd></div>
                                <div><dt class="mono">"FLEET"</dt><dd>{d.device.fleet_name.clone().unwrap_or_default()}</dd></div>
                                <div><dt class="mono">"TAGS"</dt><dd class="mono">{d.device.tags.clone()}</dd></div>
                                <div><dt class="mono">"CLAIM CODE"</dt><dd class="mono">{d.claim_code.clone().unwrap_or_default()}</dd></div>
                                <div><dt class="mono">"TWIN VERSION"</dt><dd class="mono">{d.device.twin_version}</dd></div>
                                <div><dt class="mono">"DEVICE ID"</dt><dd class="mono">{d.device.id.clone()}</dd></div>
                            </dl>
                        </section>
                    </div>
                }.into_any()
            }
        }}
    }
}

// Project lng/lat (Azerbaijan bounding box) into the 960x560 map viewBox.
fn project(lng: f64, lat: f64) -> (f64, f64) {
    let x = (lng - 44.5) / 6.0 * 880.0 + 40.0;
    let y = 520.0 - (lat - 38.3) / 3.6 * 480.0;
    (x, y)
}

#[component]
fn FleetMap() -> impl IntoView {
    let live = use_live();
    let devices = RwSignal::new(Vec::<Device>::new());
    spawn_local(async move {
        if let Ok(d) = api::devices().await {
            devices.set(d);
        }
    });
    let navigate = use_navigate();

    view! {
        <div class="page-head"><div>
            <h1 class="page-title">"Fleet Map"</h1>
            <p class="page-desc">"Every device on the ground — live position and status."</p>
        </div></div>
        <div class="panel map-panel">
            <svg viewBox="0 0 960 560" class="fleetmap">
                {[("Baku", 49.87, 40.40), ("Ganja", 46.36, 40.68), ("Sumqayit", 49.67, 40.59)].iter().map(|(n, lng, lat)| {
                    let (x, y) = project(*lng, *lat);
                    view! { <text x=x y=y class="map-city mono">{*n}</text> }
                }).collect_view()}
                {move || {
                    let tele = live.telemetry.get();
                    let navigate = navigate.clone();
                    devices.get().into_iter().filter_map(|d| {
                        let dl = tele.devices.get(&d.id)?.clone();
                        let lat = *dl.metrics.get("lat")?;
                        let lng = *dl.metrics.get("lng")?;
                        let (x, y) = project(lng, lat);
                        let tone = lamp_tone(&d, &dl);
                        let id = d.id.clone();
                        let name = d.name.clone();
                        let navigate = navigate.clone();
                        Some(view! {
                            <g class="map-node" transform=format!("translate({:.1},{:.1})", x, y) on:click=move |_| navigate(&format!("/devices/{}", id), Default::default())>
                                <circle r="6" class=format!("map-dot map-{}", tone)/>
                                <text y="-11" class="map-label">{name}</text>
                            </g>
                        })
                    }).collect_view()
                }}
            </svg>
        </div>
    }
}

fn area_path(series: &[f64], w: f64, h: f64) -> (String, String) {
    let max = series.iter().cloned().fold(1.0_f64, f64::max);
    let n = series.len();
    let pts: Vec<(f64, f64)> = series
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let x = i as f64 / (n - 1) as f64 * w;
            let y = h - (v / max) * (h - 10.0) - 5.0;
            (x, y)
        })
        .collect();
    let line = pts
        .iter()
        .enumerate()
        .map(|(i, (x, y))| format!("{}{:.1} {:.1}", if i == 0 { "M" } else { "L" }, x, y))
        .collect::<Vec<_>>()
        .join(" ");
    let area = format!("{} L {:.1} {:.1} L 0 {:.1} Z", line, w, h, h);
    (line, area)
}

#[component]
fn Analytics() -> impl IntoView {
    let live = use_live();
    let devices = RwSignal::new(Vec::<Device>::new());
    spawn_local(async move {
        if let Ok(d) = api::devices().await {
            devices.set(d);
        }
    });
    let agg = move || live.telemetry.get().aggregate;
    let export = move |_| {
        if let Some(w) = web_sys::window() {
            let _ = w.location().set_href("/api/export/devices.csv");
        }
    };

    view! {
        <div class="page-head"><div>
            <h1 class="page-title">"Analytics"</h1>
            <p class="page-desc">"Fleet throughput, availability, and distribution."</p>
        </div></div>
        <div class="kpi-grid">
            <div class="stat stat-amber"><div class="stat-label mono">"TELEMETRY RATE"</div><div class="stat-value">{move || format!("{:.0}", agg().msg_rate)}<span class="stat-unit mono">" msg/s"</span></div></div>
            <div class="stat stat-up"><div class="stat-label mono">"ONLINE"</div><div class="stat-value">{move || agg().online}<span class="stat-unit mono">{move || format!(" / {}", agg().total)}</span></div></div>
            <div class=move || format!("stat stat-{}", if agg().alerts > 0 { "alarm" } else { "up" })><div class="stat-label mono">"OPEN FAULTS"</div><div class="stat-value">{move || agg().alerts}</div></div>
            <div class="stat stat-up"><div class="stat-label mono">"AVAILABILITY"</div><div class="stat-value">{move || { let a = agg(); if a.total == 0 { "100.0".to_string() } else { format!("{:.1}", a.online as f64 / a.total as f64 * 100.0) } }}<span class="stat-unit mono">" %"</span></div></div>
        </div>
        <section class="panel section-block">
            <div class="panel-head"><div class="panel-title">"Throughput"</div><div class="panel-note mono">"live · msg/s"</div></div>
            {move || {
                let s = live.series.get();
                if s.len() < 2 {
                    return view! { <div class="chart-empty mono">"gathering samples…"</div> }.into_any();
                }
                let (line, area) = area_path(&s, 640.0, 130.0);
                view! { <svg viewBox="0 0 640 130" class="chart" preserveAspectRatio="none"><path d=area class="chart-area"/><path d=line class="chart-line"/></svg> }.into_any()
            }}
        </section>
        <section class="panel section-block">
            <div class="panel-head"><div class="panel-title">"Fleet availability"</div><button class="btn btn-inline" on:click=export>"Export CSV"</button></div>
            <div class="bars">
                {move || {
                    let tele = live.telemetry.get();
                    let mut fleets: std::collections::BTreeMap<String, (u32, u32)> = std::collections::BTreeMap::new();
                    for d in devices.get() {
                        let online = tele.devices.get(&d.id).map(|l| l.online).unwrap_or(false);
                        let e = fleets.entry(d.fleet_name.clone().unwrap_or_else(|| "Unassigned".into())).or_default();
                        e.1 += 1;
                        if online {
                            e.0 += 1;
                        }
                    }
                    fleets.into_iter().map(|(name, (on, total))| {
                        let pct = if total > 0 { on * 100 / total } else { 0 };
                        view! {
                            <div class="bar-row">
                                <span class="bar-label">{name}</span>
                                <div class="bar-track"><div class="bar-fill" style=format!("width:{}%", pct)></div></div>
                                <span class="bar-val mono">{on}" / "{total}</span>
                            </div>
                        }
                    }).collect_view().into_any()
                }}
            </div>
        </section>
    }
}

#[component]
fn Settings() -> impl IntoView {
    let auth = use_auth();
    let theme = use_context::<ThemeCtx>().expect("ThemeCtx");
    let (cur, set_cur) = signal(String::new());
    let (next, set_next) = signal(String::new());
    let (msg, set_msg) = signal(String::new());
    let (wh, set_wh) = signal(String::new());
    let (wh_msg, set_wh_msg) = signal(String::new());
    spawn_local(async move {
        if let Ok(v) = api::get_webhook().await {
            if let Some(u) = v.get("url").and_then(|u| u.as_str()) {
                set_wh.set(u.to_string());
            }
        }
    });
    let can_write = move || auth.role.get() != "viewer";

    let change_pass = move |_| {
        let (c, n) = (cur.get(), next.get());
        if n.len() < 6 {
            set_msg.set("New passcode must be at least 6 characters".into());
            return;
        }
        spawn_local(async move {
            match api::change_passcode(&c, &n).await {
                Ok(_) => {
                    set_msg.set("Passcode updated".into());
                    set_cur.set(String::new());
                    set_next.set(String::new());
                }
                Err(e) => set_msg.set(e),
            }
        });
    };
    let save_wh = move |_| {
        let u = wh.get();
        spawn_local(async move {
            match api::set_webhook(&u).await {
                Ok(_) => set_wh_msg.set("Webhook saved".into()),
                Err(e) => set_wh_msg.set(e),
            }
        });
    };
    let backup = move |_| {
        if let Some(w) = web_sys::window() {
            let _ = w.location().set_href("/api/backup");
        }
    };

    view! {
        <div class="page-head"><div>
            <h1 class="page-title">"Settings"</h1>
            <p class="page-desc">"Console appearance, security, and integrations."</p>
        </div></div>
        <div class="set-grid">
            <div class="panel">
                <div class="panel-title">"Appearance & data"</div>
                <div class="set-row">
                    <div><div class="set-name">"Theme"</div><div class="set-sub">"Operations bunker (dark) or field manual (light)."</div></div>
                    <div class="row gap-2">
                        <button class=move || if theme.theme.get() == "light" { "btn btn-inline btn-primary" } else { "btn btn-inline" } on:click=move |_| theme.set.set("light".into())>"Light"</button>
                        <button class=move || if theme.theme.get() == "dark" { "btn btn-inline btn-primary" } else { "btn btn-inline" } on:click=move |_| theme.set.set("dark".into())>"Dark"</button>
                    </div>
                </div>
                <div class="set-row">
                    <div><div class="set-name">"Backup"</div><div class="set-sub">"Download a clean SQLite snapshot."</div></div>
                    <button class="btn btn-inline" on:click=backup>"Download"</button>
                </div>
            </div>
            {move || can_write().then(|| view! {
                <div class="panel">
                    <div class="panel-title">"Security"</div>
                    <label class="field"><span class="field-label mono">"CURRENT PASSCODE"</span><input class="input mono" type="password" prop:value=move || cur.get() on:input=move |e| set_cur.set(event_target_value(&e))/></label>
                    <label class="field"><span class="field-label mono">"NEW PASSCODE"</span><input class="input mono" type="password" prop:value=move || next.get() on:input=move |e| set_next.set(event_target_value(&e))/></label>
                    <button class="btn btn-primary btn-inline set-btn" on:click=change_pass>"Update passcode"</button>
                    {move || (!msg.get().is_empty()).then(|| view! { <div class="cfg-msg mono">{msg.get()}</div> })}
                </div>
            })}
            {move || can_write().then(|| view! {
                <div class="panel">
                    <div class="panel-title">"Alert webhook"</div>
                    <p class="set-sub">"Endpoint notified when alerts fire (integration point)."</p>
                    <label class="field"><span class="field-label mono">"ENDPOINT URL"</span><input class="input mono" type="url" placeholder="https://hooks.example.com/switchboard" prop:value=move || wh.get() on:input=move |e| set_wh.set(event_target_value(&e))/></label>
                    <button class="btn btn-primary btn-inline set-btn" on:click=save_wh>"Save webhook"</button>
                    {move || (!wh_msg.get().is_empty()).then(|| view! { <div class="cfg-msg mono">{wh_msg.get()}</div> })}
                </div>
            })}
        </div>
    }
}

#[component]
fn Fleets() -> impl IntoView {
    let auth = use_auth();
    let fleets = RwSignal::new(Vec::<api::Fleet>::new());
    let reload = move || {
        spawn_local(async move {
            if let Ok(f) = api::fleets().await {
                fleets.set(f);
            }
        });
    };
    reload();
    let (name, set_name) = signal(String::new());
    let (desc, set_desc) = signal(String::new());
    let can_write = move || auth.role.get() != "viewer";
    let create = move |_| {
        let (n, d) = (name.get(), desc.get());
        if n.trim().is_empty() {
            return;
        }
        spawn_local(async move {
            let _ = api::create_fleet(&n, &d).await;
            set_name.set(String::new());
            set_desc.set(String::new());
            reload();
        });
    };
    let remove = move |id: String| {
        spawn_local(async move {
            let _ = api::delete_fleet(&id).await;
            reload();
        });
    };

    view! {
        <div class="page-head"><div>
            <h1 class="page-title">"Fleets"</h1>
            <p class="page-desc">"Group devices into fleets for config and rollout targeting."</p>
        </div></div>
        {move || can_write().then(|| view! {
            <div class="panel section-block">
                <div class="panel-title">"New fleet"</div>
                <div class="form-row cfg-actions">
                    <input class="input mono" placeholder="Fleet name" prop:value=move || name.get() on:input=move |e| set_name.set(event_target_value(&e))/>
                    <input class="input mono" placeholder="Description" prop:value=move || desc.get() on:input=move |e| set_desc.set(event_target_value(&e))/>
                    <button class="btn btn-primary btn-inline" on:click=create>"Create fleet"</button>
                </div>
            </div>
        })}
        <div class="cfg-grid">
            {move || fleets.get().into_iter().map(|f| {
                let can = can_write();
                let id = f.id.clone();
                view! {
                    <div class="cfg-card">
                        <div class="between"><div class="cfg-name">{f.name.clone()}</div><span class="pill pill-state-acked">{f.device_count}" devices"</span></div>
                        <div class="set-sub" style="margin:6px 0 10px">{f.description.clone()}</div>
                        {can.then(|| { let id = id.clone(); view! { <button class="mini-btn" on:click=move |_| remove(id.clone())>"Delete"</button> } })}
                    </div>
                }
            }).collect_view()}
        </div>
    }
}

#[component]
fn Team() -> impl IntoView {
    let auth = use_auth();
    let ops = RwSignal::new(Vec::<api::Operator>::new());
    let reload = move || {
        spawn_local(async move {
            if let Ok(o) = api::operators().await {
                ops.set(o);
            }
        });
    };
    reload();
    let (name, set_name) = signal(String::new());
    let (email, set_email) = signal(String::new());
    let (role, set_role) = signal(String::from("operator"));
    let (pw, set_pw) = signal(String::new());
    let (msg, set_msg) = signal(String::new());
    let can_manage = move || matches!(auth.role.get().as_str(), "owner" | "admin");

    let create = move |_| {
        let (n, e, r, p) = (name.get(), email.get(), role.get(), pw.get());
        if n.trim().is_empty() || e.trim().is_empty() || p.len() < 6 {
            set_msg.set("Name, email, and a 6+ character password are required".into());
            return;
        }
        set_msg.set(String::new());
        spawn_local(async move {
            match api::create_operator(&n, &e, &r, &p).await {
                Ok(_) => {
                    set_name.set(String::new());
                    set_email.set(String::new());
                    set_pw.set(String::new());
                    reload();
                }
                Err(e) => set_msg.set(e),
            }
        });
    };
    let remove = move |id: String| {
        spawn_local(async move {
            let _ = api::delete_operator(&id).await;
            reload();
        });
    };

    view! {
        <div class="page-head"><div>
            <h1 class="page-title">"Team"</h1>
            <p class="page-desc">"Console operators and their access roles."</p>
        </div></div>
        <div class="role-legend mono">
            <span><b>"Owner / Admin"</b>" — full access + manage team"</span>
            <span><b>"Operator"</b>" — full access"</span>
            <span><b>"Viewer"</b>" — read-only"</span>
        </div>
        {move || can_manage().then(|| view! {
            <div class="panel section-block">
                <div class="panel-title">"Invite operator"</div>
                <div class="form-row cfg-actions">
                    <input class="input mono" placeholder="Name" prop:value=move || name.get() on:input=move |e| set_name.set(event_target_value(&e))/>
                    <input class="input mono" placeholder="email@switchboard.local" prop:value=move || email.get() on:input=move |e| set_email.set(event_target_value(&e))/>
                    <select class="lvl-select mono" on:change=move |e| set_role.set(event_target_value(&e))>
                        <option value="operator">"operator"</option>
                        <option value="admin">"admin"</option>
                        <option value="viewer">"viewer"</option>
                    </select>
                    <input class="input mono" type="password" placeholder="password (6+)" prop:value=move || pw.get() on:input=move |e| set_pw.set(event_target_value(&e))/>
                    <button class="btn btn-primary btn-inline" on:click=create>"Invite"</button>
                </div>
                {move || (!msg.get().is_empty()).then(|| view! { <div class="cfg-msg mono">{msg.get()}</div> })}
            </div>
        })}
        <div class="dtable">
            <div class="dt-head mono team-cols"><span>"STATE"</span><span>"OPERATOR"</span><span>"EMAIL"</span><span>"ROLE"</span><span></span></div>
            {move || ops.get().into_iter().map(|o| {
                let can = can_manage() && o.role != "owner";
                let id = o.id.clone();
                view! {
                    <div class="dt-row team-cols cmd-row">
                        <span><span class=if o.status == "active" { "dot dot-up" } else { "dot dot-idle" }></span>{o.status.clone()}</span>
                        <span class="cell-strong">{o.name.clone()}</span>
                        <span class="mono u-muted">{o.email.clone()}</span>
                        <span class="mono">{o.role.to_uppercase()}</span>
                        <span>{can.then(|| { let id = id.clone(); view! { <button class="mini-btn" on:click=move |_| remove(id.clone())>"Remove"</button> } })}</span>
                    </div>
                }
            }).collect_view()}
        </div>
    }
}

#[component]
fn Config() -> impl IntoView {
    let auth = use_auth();
    let profiles = RwSignal::new(Vec::<api::ConfigProfile>::new());
    let devices = RwSignal::new(Vec::<Device>::new());
    let reload = move || {
        spawn_local(async move {
            if let Ok(p) = api::config_profiles().await {
                profiles.set(p);
            }
        });
    };
    reload();
    spawn_local(async move {
        if let Ok(d) = api::devices().await {
            devices.set(d);
        }
    });
    let (name, set_name) = signal(String::new());
    let (vals, set_vals) = signal(String::from("{ \"reportInterval\": 30 }"));
    let (msg, set_msg) = signal(String::new());
    let (apply_dev, set_apply_dev) = signal(String::new());
    let can_write = move || auth.role.get() != "viewer";

    let create = move |_| {
        let n = name.get();
        if n.trim().is_empty() {
            set_msg.set("Name is required".into());
            return;
        }
        let v = match serde_json::from_str::<serde_json::Value>(&vals.get()) {
            Ok(v) => v,
            Err(e) => {
                set_msg.set(format!("Invalid JSON: {e}"));
                return;
            }
        };
        set_msg.set(String::new());
        spawn_local(async move {
            let _ = api::create_profile(&n, v).await;
            set_name.set(String::new());
            reload();
        });
    };
    let apply = move |pid: String| {
        let dev = apply_dev.get();
        if dev.is_empty() {
            set_msg.set("Pick an apply target first".into());
            return;
        }
        spawn_local(async move {
            match api::apply_profile(&pid, &dev).await {
                Ok(_) => set_msg.set("Profile pushed to the device twin".into()),
                Err(e) => set_msg.set(e),
            }
        });
    };

    view! {
        <div class="page-head"><div>
            <h1 class="page-title">"Config profiles"</h1>
            <p class="page-desc">"Reusable desired-state you can push to a device twin."</p>
        </div></div>
        {move || can_write().then(|| view! {
            <div class="panel section-block">
                <div class="panel-title">"New profile"</div>
                <div class="form-row"><input class="input mono" placeholder="Profile name" prop:value=move || name.get() on:input=move |e| set_name.set(event_target_value(&e))/></div>
                <textarea class="input mono cfg-json" prop:value=move || vals.get() on:input=move |e| set_vals.set(event_target_value(&e))></textarea>
                <div class="form-row cfg-actions">
                    <button class="btn btn-primary btn-inline" on:click=create>"Create profile"</button>
                    <select class="lvl-select mono" on:change=move |e| set_apply_dev.set(event_target_value(&e))>
                        <option value="">"apply target…"</option>
                        {move || devices.get().into_iter().map(|d| view! { <option value=d.id.clone()>{d.name.clone()}</option> }).collect_view()}
                    </select>
                </div>
                {move || (!msg.get().is_empty()).then(|| view! { <div class="cfg-msg mono">{msg.get()}</div> })}
            </div>
        })}
        <div class="cfg-grid">
            {move || profiles.get().into_iter().map(|p| {
                let pid = p.id.clone();
                let can = can_write();
                view! {
                    <div class="cfg-card">
                        <div class="cfg-name">{p.name.clone()}</div>
                        <pre class="cfg-vals mono">{serde_json::to_string_pretty(&p.values).unwrap_or_default()}</pre>
                        {can.then(|| { let pid = pid.clone(); view! { <button class="mini-btn" on:click=move |_| apply(pid.clone())>"Apply to target"</button> } })}
                    </div>
                }
            }).collect_view()}
        </div>
    }
}

#[component]
fn Firmware() -> impl IntoView {
    let auth = use_auth();
    let fw = RwSignal::new(Vec::<api::Firmware>::new());
    let camps = RwSignal::new(Vec::<api::OtaCampaign>::new());
    let fleets = RwSignal::new(Vec::<api::Fleet>::new());
    let reload = move || {
        spawn_local(async move { if let Ok(f) = api::firmware().await { fw.set(f); } });
        spawn_local(async move { if let Ok(c) = api::campaigns().await { camps.set(c); } });
    };
    reload();
    spawn_local(async move { if let Ok(f) = api::fleets().await { fleets.set(f); } });

    // Poll campaign progress while mounted.
    let alive = RwSignal::new(true);
    on_cleanup(move || alive.set(false));
    spawn_local(async move {
        while alive.get_untracked() {
            gloo_timers::future::TimeoutFuture::new(2000).await;
            if !alive.get_untracked() {
                break;
            }
            reload();
        }
    });

    let (model, set_model) = signal(String::new());
    let (version, set_version) = signal(String::new());
    let (sel_fw, set_sel_fw) = signal(String::new());
    let (sel_fleet, set_sel_fleet) = signal(String::new());
    let (canary, set_canary) = signal(100_i64);
    let can_write = move || auth.role.get() != "viewer";

    let register = move |_| {
        let (m, v) = (model.get(), version.get());
        if m.trim().is_empty() || v.trim().is_empty() {
            return;
        }
        spawn_local(async move {
            let _ = api::create_firmware(&m, &v).await;
            set_model.set(String::new());
            set_version.set(String::new());
            reload();
        });
    };
    let start = move |_| {
        let f = sel_fw.get();
        if f.is_empty() {
            return;
        }
        let fleet = {
            let s = sel_fleet.get();
            if s.is_empty() { None } else { Some(s) }
        };
        let cn = canary.get();
        spawn_local(async move {
            let _ = api::create_campaign(&f, fleet, cn).await;
            reload();
        });
    };

    view! {
        <div class="page-head"><div>
            <h1 class="page-title">"Firmware & OTA"</h1>
            <p class="page-desc">"Register firmware artifacts and roll them out with canary campaigns."</p>
        </div></div>
        {move || can_write().then(|| view! {
            <div class="panel section-block">
                <div class="panel-title">"Register firmware"</div>
                <div class="form-row cfg-actions">
                    <input class="input mono" placeholder="Model (e.g. AeroTherm X3)" prop:value=move || model.get() on:input=move |e| set_model.set(event_target_value(&e))/>
                    <input class="input mono" placeholder="Version (e.g. 2.5.0)" prop:value=move || version.get() on:input=move |e| set_version.set(event_target_value(&e))/>
                    <button class="btn btn-primary btn-inline" on:click=register>"Register"</button>
                </div>
                <div class="panel-title" style="margin-top:18px">"Start rollout"</div>
                <div class="form-row cfg-actions">
                    <select class="lvl-select mono" on:change=move |e| set_sel_fw.set(event_target_value(&e))>
                        <option value="">"firmware…"</option>
                        {move || fw.get().into_iter().map(|f| view! { <option value=f.id.clone()>{format!("{} {}", f.model, f.version)}</option> }).collect_view()}
                    </select>
                    <select class="lvl-select mono" on:change=move |e| set_sel_fleet.set(event_target_value(&e))>
                        <option value="">"all fleets"</option>
                        {move || fleets.get().into_iter().map(|f| view! { <option value=f.id.clone()>{f.name.clone()}</option> }).collect_view()}
                    </select>
                    <select class="lvl-select mono" on:change=move |e| set_canary.set(event_target_value(&e).parse().unwrap_or(100))>
                        <option value="100">"100% (all)"</option>
                        <option value="50">"50% canary"</option>
                        <option value="25">"25% canary"</option>
                        <option value="10">"10% canary"</option>
                    </select>
                    <button class="btn btn-primary btn-inline" on:click=start>"Start rollout"</button>
                </div>
            </div>
        })}

        <section class="panel section-block">
            <div class="panel-head"><div class="panel-title">"Rollouts"</div></div>
            <div class="ota-list">
                {move || {
                    let items = camps.get();
                    if items.is_empty() {
                        return view! { <div class="empty">"No rollouts yet."</div> }.into_any();
                    }
                    items.into_iter().map(|c| {
                        let pct = if c.total > 0 { (c.updated * 100 / c.total).min(100) } else { 0 };
                        view! {
                            <div class="ota-row">
                                <div class="ota-head">
                                    <span class="ota-fw">{c.firmware_label.clone().unwrap_or_else(|| "firmware".into())}</span>
                                    <span class="ota-fleet mono">{c.fleet_name.clone().unwrap_or_else(|| "all fleets".into())}" · "{c.canary_pct}"%"</span>
                                    <span class=format!("pill pill-state-{}", if c.status == "completed" { "resolved" } else { "acked" })>{c.status.clone()}</span>
                                </div>
                                <div class="ota-bar"><div class="ota-fill" style=format!("width:{}%", pct)></div></div>
                                <div class="ota-meta mono">{c.updated}" / "{c.total}" devices"</div>
                            </div>
                        }
                    }).collect_view().into_any()
                }}
            </div>
        </section>

        <section class="panel section-block">
            <div class="panel-head"><div class="panel-title">"Firmware registry"</div></div>
            <div class="dtable">
                <div class="dt-head mono fw-cols"><span>"MODEL"</span><span>"VERSION"</span><span>"SIZE"</span><span>"SHA-256"</span></div>
                {move || fw.get().into_iter().map(|f| view! {
                    <div class="dt-row fw-cols" style="cursor:default">
                        <span class="cell-strong">{f.model.clone()}</span>
                        <span class="mono">{f.version.clone()}</span>
                        <span class="mono u-muted">{format!("{} KB", f.size_kb)}</span>
                        <span class="mono u-muted">{f.sha256.clone()}</span>
                    </div>
                }).collect_view()}
            </div>
        </section>
    }
}

#[component]
fn Commands() -> impl IntoView {
    let auth = use_auth();
    let devices = RwSignal::new(Vec::<Device>::new());
    let cmds = RwSignal::new(Vec::<api::Command>::new());
    let (sel, set_sel) = signal(String::new());
    spawn_local(async move {
        if let Ok(d) = api::devices().await {
            if let Some(first) = d.first() {
                set_sel.set(first.id.clone());
            }
            devices.set(d);
        }
    });
    let reload = move || {
        spawn_local(async move {
            if let Ok(c) = api::commands().await {
                cmds.set(c);
            }
        });
    };
    reload();
    let can_write = move || auth.role.get() != "viewer";
    let send = move |name: &'static str| {
        let device = sel.get();
        if device.is_empty() {
            return;
        }
        spawn_local(async move {
            let _ = api::send_command(&device, name).await;
            gloo_timers::future::TimeoutFuture::new(2600).await;
            reload();
        });
    };

    view! {
        <div class="page-head"><div>
            <h1 class="page-title">"Commands"</h1>
            <p class="page-desc">"Send remote commands and review the response history."</p>
        </div></div>
        {move || can_write().then(|| view! {
            <div class="cmd-bar">
                <select class="lvl-select mono" on:change=move |e| set_sel.set(event_target_value(&e))>
                    {move || devices.get().into_iter().map(|d| view! { <option value=d.id.clone()>{d.name.clone()}</option> }).collect_view()}
                </select>
                <button class="mini-btn" on:click=move |_| send("reboot")>"Reboot"</button>
                <button class="mini-btn" on:click=move |_| send("ping")>"Ping"</button>
                <button class="mini-btn" on:click=move |_| send("sync")>"Sync"</button>
                <button class="mini-btn" on:click=move |_| send("identify")>"Identify"</button>
            </div>
        })}
        <div class="dtable">
            <div class="dt-head mono cmd-cols"><span>"STATUS"</span><span>"DEVICE"</span><span>"COMMAND"</span><span>"RESULT"</span></div>
            {move || cmds.get().into_iter().map(|c| {
                let pill = if c.status == "completed" { "resolved" } else if c.status == "failed" { "open" } else { "acked" };
                view! {
                    <div class="dt-row cmd-cols cmd-row">
                        <span><span class=format!("pill pill-state-{}", pill)>{c.status.clone()}</span></span>
                        <span class="cell-strong">{c.device_name.clone().unwrap_or_default()}</span>
                        <span class="mono">{c.name.clone()}</span>
                        <span class="u-muted">{c.result.clone()}</span>
                    </div>
                }
            }).collect_view()}
        </div>
    }
}

#[component]
fn Logs() -> impl IntoView {
    let live = use_live();
    spawn_local(async move {
        if let Ok(l) = api::logs().await {
            live.logs.set(l);
        }
    });
    let (q, set_q) = signal(String::new());
    let (level, set_level) = signal(String::from("all"));

    view! {
        <div class="page-head"><div>
            <h1 class="page-title">"Logs"</h1>
            <p class="page-desc">"Live device log stream — newest first."</p>
        </div></div>
        <div class="toolbar">
            <input class="search mono" placeholder="Filter logs…" prop:value=move || q.get() on:input=move |e| set_q.set(event_target_value(&e))/>
            <select class="lvl-select mono" on:change=move |e| set_level.set(event_target_value(&e))>
                <option value="all">"all levels"</option>
                <option value="info">"info"</option>
                <option value="warning">"warning"</option>
                <option value="error">"error"</option>
            </select>
        </div>
        <div class="logstream">
            {move || {
                let needle = q.get().to_lowercase();
                let lv = level.get();
                live.logs.get().into_iter()
                    .filter(|l| (lv == "all" || l.level == lv)
                        && (needle.is_empty() || format!("{} {}", l.msg, l.device_name.clone().unwrap_or_default()).to_lowercase().contains(&needle)))
                    .map(|l| view! {
                        <div class="logline mono">
                            <span class=format!("log-lvl log-{}", l.level)>{l.level.clone()}</span>
                            <span class="log-dev">{l.device_name.clone().unwrap_or_default()}</span>
                            <span class="log-msg">{l.msg.clone()}</span>
                        </div>
                    }).collect_view()
            }}
        </div>
    }
}

#[component]
fn Rules() -> impl IntoView {
    let rules = [
        ("critical", "Device offline", "No telemetry received within the keepalive window", "Raise a critical alert; auto-resolve on reconnect"),
        ("warning", "High temperature", "tempC exceeds 30 °C", "Raise a warning alert; auto-resolve when it cools"),
        ("warning", "Low battery", "batteryPct drops below 15%", "Raise a warning alert; auto-resolve on charge"),
    ];
    view! {
        <div class="page-head"><div>
            <h1 class="page-title">"Rules"</h1>
            <p class="page-desc">"Conditions the telemetry engine evaluates to raise alerts."</p>
        </div></div>
        <div class="cfg-grid">
            {rules.iter().map(|(sev, name, cond, action)| {
                let dot = if *sev == "critical" { "down" } else { "warn" };
                view! {
                    <div class="cfg-card">
                        <div class="between"><div class="cfg-name">{*name}</div><span class=format!("dot dot-{}", dot)></span></div>
                        <div class="rule-cond mono">{*cond}</div>
                        <div class="rule-action">{*action}</div>
                    </div>
                }
            }).collect_view()}
        </div>
        <p class="scaffold-note mono" style="margin-top:18px">"Built-in rules evaluate every ~30s. Custom user-defined rules are on the roadmap."</p>
    }
}

#[component]
fn Alerts() -> impl IntoView {
    let auth = use_auth();
    let alerts = RwSignal::new(Vec::<api::Alert>::new());
    let reload = move || {
        spawn_local(async move {
            if let Ok(a) = api::alerts().await {
                alerts.set(a);
            }
        });
    };
    reload();
    let can_write = move || auth.role.get() != "viewer";

    view! {
        <div class="page-head"><div>
            <h1 class="page-title">"Alerts"</h1>
            <p class="page-desc">"Faults raised across the fleet — acknowledge and resolve."</p>
        </div></div>
        <div class="alert-list">
            {move || {
                let items = alerts.get();
                if items.is_empty() {
                    return view! { <div class="empty">"No alerts. All clear."</div> }.into_any();
                }
                items.into_iter().map(|a| {
                    let sev = match a.severity.as_str() { "critical" => "down", "warning" => "warn", _ => "idle" };
                    let can = can_write() && a.state != "resolved";
                    let is_open = a.state == "open";
                    let id = a.id.clone();
                    view! {
                        <div class=format!("alert-row alert-{}", a.state)>
                            <span class=format!("dot dot-{}", sev)></span>
                            <div class="grow">
                                <div class="alert-title">{a.title.clone()}" "<span class="alert-dev mono">{a.device_name.clone().unwrap_or_default()}</span></div>
                                <div class="alert-detail">{a.detail.clone()}</div>
                            </div>
                            <span class=format!("pill pill-state-{}", a.state)>{a.state.clone()}</span>
                            {can.then(|| {
                                let id_a = id.clone();
                                let id_r = id.clone();
                                view! {
                                    <div class="alert-acts">
                                        {is_open.then(|| view! { <button class="mini-btn" on:click=move |_| { let id = id_a.clone(); spawn_local(async move { let _ = api::alert_action(&id, "ack").await; reload(); }); }>"Ack"</button> })}
                                        <button class="mini-btn" on:click=move |_| { let id = id_r.clone(); spawn_local(async move { let _ = api::alert_action(&id, "resolve").await; reload(); }); }>"Resolve"</button>
                                    </div>
                                }
                            })}
                        </div>
                    }
                }).collect_view().into_any()
            }}
        </div>
    }
}

/// The patch-bay jack glyph — the product mark.
#[component]
fn Jack() -> impl IntoView {
    view! {
        <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round">
            <circle cx="12" cy="12" r="8"/>
            <circle cx="12" cy="12" r="2.6" fill="currentColor" stroke="none"/>
            <path d="M12 2v3M12 19v3"/>
        </svg>
    }
}
