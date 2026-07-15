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
                    </Routes>
                </main>
            </div>
        </Router>
    }
}

const NAV: &[(&str, &[(&str, &str, &str)])] = &[
    ("Operations", &[("Overview", "OV", "/"), ("Fleet Map", "MP", "#")]),
    ("Fleet", &[("Devices", "DV", "/devices"), ("Fleets", "FL", "#")]),
    ("Delivery", &[("Config", "CF", "#"), ("Firmware", "FW", "#"), ("Commands", "CM", "/commands")]),
    ("Observe", &[("Logs", "LG", "/logs"), ("Rules", "RL", "#"), ("Alerts", "AL", "/alerts")]),
    ("Insights", &[("Analytics", "AN", "#")]),
    ("Admin", &[("Team", "TM", "#"), ("Settings", "ST", "#")]),
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
