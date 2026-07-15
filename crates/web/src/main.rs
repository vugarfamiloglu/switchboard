//! Switchboard console — Leptos CSR app. Phase 0: the house app-shell (sidebar +
//! sticky topbar + workbench) in the "Field Operations" design, with a static
//! Overview. Live data, routing, and per-module views arrive in Phase 1.

use leptos::prelude::*;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
    let (theme, set_theme) = signal(String::from("dark"));
    view! {
        <div class=move || format!("app theme-{}", theme.get())>
            <Sidebar/>
            <TopBar theme set_theme/>
            <main class="workbench">
                <Overview/>
            </main>
        </div>
    }
}

const NAV: &[(&str, &[(&str, &str)])] = &[
    ("Operations", &[("Overview", "OV"), ("Patch Bay", "PB"), ("Fleet Map", "MP")]),
    ("Fleet", &[("Devices", "DV"), ("Fleets", "FL")]),
    ("Delivery", &[("Config", "CF"), ("Firmware", "FW"), ("Commands", "CM")]),
    ("Observe", &[("Logs", "LG"), ("Rules", "RL"), ("Alerts", "AL")]),
    ("Insights", &[("Analytics", "AN")]),
    ("Admin", &[("Team", "TM"), ("Settings", "ST")]),
];

#[component]
fn Sidebar() -> impl IntoView {
    view! {
        <aside class="sidebar">
            <div class="brand">
                <span class="brand-mark"><Jack/></span>
                <span class="brand-word">"Switch" <b>"board"</b></span>
            </div>
            <nav class="nav">
                {NAV.iter().map(|(group, items)| view! {
                    <div class="nav-group">
                        <div class="nav-group-label mono">{*group}</div>
                        {items.iter().enumerate().map(|(i, (label, glyph))| view! {
                            <a href="#" class=move || if *group == "Operations" && i == 0 { "nav-link is-active" } else { "nav-link" }>
                                <span class="nav-glyph mono">{*glyph}</span>
                                <span class="nav-label">{*label}</span>
                            </a>
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
    let toggle = move |_| {
        set_theme.update(|t| *t = if t == "dark" { "light".into() } else { "dark".into() });
    };
    view! {
        <header class="topbar">
            <div class="tb-left">
                <span class="tb-mark"><Jack/></span>
                <span class="tb-crumb mono">"OPERATIONS"</span>
                <span class="tb-div"></span>
                <div class="tb-titles">
                    <span class="tb-title">"Control Overview"</span>
                    <span class="tb-sub">"Live estate posture"</span>
                </div>
            </div>
            <div class="tb-badge" title="Aggregate telemetry rate">
                <span class="tb-badge-dot"></span>
                <span class="tb-badge-val mono">"2,418"</span>
                <span class="tb-badge-unit mono">"msg/s"</span>
            </div>
            <div class="tb-right">
                <button class="tb-icon-btn" on:click=toggle title="Toggle theme">
                    {move || if theme.get() == "dark" { "☀" } else { "☾" }}
                </button>
            </div>
        </header>
    }
}

#[component]
fn Overview() -> impl IntoView {
    let tiles = [
        ("Devices online", "128", "/ 134", "up"),
        ("Telemetry rate", "2,418", "msg/s", "amber"),
        ("Open alerts", "3", "needs triage", "alarm"),
        ("Fleet uptime", "99.2", "% · 30d", "up"),
    ];
    // A teaser of the patch-bay wall: each cell is a device jack with a status lamp.
    let lamps = (0..48)
        .map(|i| match i {
            7 | 33 => "alarm",
            3 | 19 | 40 => "amber",
            _ => "up",
        })
        .collect::<Vec<_>>();

    view! {
        <div class="page-head">
            <div>
                <h1 class="page-title">"Control Overview"</h1>
                <p class="page-desc">"Live posture across every device, fleet, and edge line."</p>
            </div>
        </div>

        <div class="kpi-grid">
            {tiles.iter().map(|(label, value, unit, tone)| view! {
                <div class=format!("stat stat-{}", tone)>
                    <div class="stat-label mono">{*label}</div>
                    <div class="stat-value">{*value} <span class="stat-unit mono">{*unit}</span></div>
                </div>
            }).collect_view()}
        </div>

        <section class="panel">
            <div class="panel-head">
                <div class="panel-title">"Patch bay"</div>
                <div class="panel-note mono">"48 lines · live lamps in Phase 1"</div>
            </div>
            <div class="patchbay">
                {lamps.into_iter().enumerate().map(|(i, tone)| view! {
                    <div class=format!("jackcell jack-{}", tone) title=move || format!("line {:02}", i + 1)>
                        <span class=format!("lamp lamp-{}", tone)></span>
                        <span class="jack-id mono">{format!("{:02}", i + 1)}</span>
                    </div>
                }).collect_view()}
            </div>
        </section>

        <p class="scaffold-note mono">
            "Phase 0 scaffold · Rust + Axum + Leptos · next: routing, device registry, MQTT ingest, live telemetry."
        </p>
    }
}

/// The patch-bay jack glyph — the product mark.
#[component]
fn Jack() -> impl IntoView {
    view! {
        <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor"
            stroke-width="1.7" stroke-linecap="round">
            <circle cx="12" cy="12" r="8"/>
            <circle cx="12" cy="12" r="2.6" fill="currentColor" stroke="none"/>
            <path d="M12 2v3M12 19v3"/>
        </svg>
    }
}
