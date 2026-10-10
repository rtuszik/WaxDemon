use crate::charts::{Chart, breakdown_link, breakdown_options, history_options};
use crate::{
    Bootstrap,
    remote::{Remote, Status, money, post, remote, text},
};
use leptos::prelude::*;
use leptos_router::{
    components::{A, Form},
    hooks::{use_location, use_params_map, use_query_map},
};
use serde_json::Value;
use waxdemon_core::library::LibraryPage;

fn csrf() -> String {
    use_context::<Bootstrap>().unwrap_or_default().csrf
}

#[component]
pub fn Login() -> impl IntoView {
    view! {
        <section class="welcome panel">
            <h1>"Sign in"</h1>
            <form action="/auth/login" method="post">
                <input type="hidden" name="csrf" value=csrf() />
                <button>"Continue with Discogs"</button>
            </form>
            <p class="muted">"New accounts require administrator approval."</p>
        </section>
    }
}

#[component]
pub fn Overview() -> impl IntoView {
    let bootstrap = use_context::<Bootstrap>().unwrap_or_default();
    if bootstrap
        .user
        .as_ref()
        .is_none_or(|u| u["status"] != "approved")
    {
        return view! {
            <section class="welcome panel">
                <h1>"Awaiting approval"</h1>
                <p>
                    "An administrator needs to approve your account before you can explore or synchronize your collection."
                </p>
                <form method="get" action="/">
                    <button>"Check approval status"</button>
                </form>
            </section>
        }.into_any();
    }
    view! { <Dashboard /> }.into_any()
}

#[component]
fn Dashboard() -> impl IntoView {
    let location = use_location();
    let data = remote(Signal::derive(move || {
        crate::remote::with_query("/api/dashboard", &location.search.get())
    }));
    view! {
        <Status remote=data />
        {move || {
            let value = data.data.get();
            let totals = value["summaries"].as_array().cloned().unwrap_or_default();
            view! {
                <section class="metrics">
                    <article class="metric panel">
                        <span>"Items"</span>
                        <strong>{value["total_items"].as_i64().unwrap_or(0)}</strong>
                    </article>
                    {totals
                        .into_iter()
                        .flat_map(|v| {
                            [
                                ("minimum", "Value (min)"),
                                ("median", "Value (median)"),
                                ("maximum", "Value (max)"),
                                ("average", "Avg / item"),
                            ]
                                .into_iter()
                                .map(move |(field, label)| {
                                    view! {
                                        <article class="metric panel">
                                            <span>{label}</span>
                                            <strong>
                                                {money(v[field].as_str(), v["currency"].as_str())}
                                            </strong>
                                            <small>
                                                {format!(
                                                    "Snapshot: {}",
                                                    text(&v, "timestamp").chars().take(10).collect::<String>(),
                                                )}
                                            </small>
                                        </article>
                                    }
                                })
                        })
                        .collect_view()}
                </section>
            }
        }}
        <div class="section-heading">
            <h2>"Over time"</h2>
            <Form method="get" action="/">
                <label>
                    "History range "
                    <select name="range">
                        {[
                            ("all", "All history"),
                            ("1m", "Last month"),
                            ("3m", "Last 3 months"),
                            ("6m", "Last 6 months"),
                            ("1y", "Last year"),
                        ]
                            .into_iter()
                            .map(|(value, label)| {
                                view! {
                                    <option
                                        value=value
                                        selected=move || {
                                            url::form_urlencoded::parse(
                                                    location.search.get().trim_start_matches('?').as_bytes(),
                                                )
                                                .find(|(k, _)| k == "range")
                                                .map(|(_, v)| v == value)
                                                .unwrap_or(value == "all")
                                        }
                                    >
                                        {label}
                                    </option>
                                }
                            })
                            .collect_view()}
                    </select>
                </label>
                <button>"Apply"</button>
            </Form>
        </div>
        <div class="two-columns">
            <HistoryChart data />
            <section class="panel">
                <h3>"Collection size"</h3>
                <Chart
                    options=Signal::derive(move || history_options(
                        data
                            .data
                            .get()["history"]
                            .as_array()
                            .map(Vec::as_slice)
                            .unwrap_or_default(),
                        "unknown",
                        true,
                    ))
                    label="Collection size with zoom and pan"
                />
                {move || {
                    data
                        .data
                        .get()["history"]
                        .as_array()
                        .is_some_and(|history| history.iter().any(|point| point["source"] == "inferred"))
                        .then_some(view! {
                            <small class="muted">
                                "Earlier points are inferred from Discogs date added and exclude records removed before the first sync."
                            </small>
                        })
                }}
            </section>
        </div>
        {move || {
            data
                .data
                .get()["rankings"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .map(|group| {
                    view! {
                        <div class="two-columns">
                            <RecordList
                                title=format!(
                                    "Top valuable · {}",
                                    group["currency"].as_str().unwrap_or("Unknown currency"),
                                )
                                items=group["top"].as_array().cloned().unwrap_or_default()
                            />
                            <RecordList
                                title=format!(
                                    "Least valuable · {}",
                                    group["currency"].as_str().unwrap_or("Unknown currency"),
                                )
                                items=group["bottom"].as_array().cloned().unwrap_or_default()
                            />
                        </div>
                    }
                })
                .collect_view()
        }}
        <div class="two-columns">
            <section class="panel">
                <h2>"Decade distribution"</h2>
                <Chart
                    options=Signal::derive(move || breakdown_options(
                        data.data.get()["decades"].as_array().map(Vec::as_slice).unwrap_or_default(),
                    ))
                    label="Collection by release decade; select a slice to filter the library"
                    filter="decade"
                />
                <div class="breakdown">
                    {move || {
                        data
                            .data
                            .get()["decades"]
                            .as_array()
                            .cloned()
                            .unwrap_or_default()
                            .into_iter()
                            .map(|v| {
                                let name = text(&v, "name");
                                view! {
                                    <div>
                                        {if let Some(href) = breakdown_link("decade", &name) {
                                            view! { <A href=href>{name}</A> }
                                                .into_any()
                                        } else {
                                            view! { <span>{name}</span> }.into_any()
                                        }}<strong>{v["count"].as_i64()}</strong>
                                    </div>
                                }
                            })
                            .collect_view()
                    }}
                </div>
            </section>
            {move || {
                view! {
                    <RecordList
                        title="Latest additions".into()
                        items=data.data.get()["additions"].as_array().cloned().unwrap_or_default()
                    />
                }
            }}
        </div>
        <div class="two-columns">
            {["genres", "formats"]
                .into_iter()
                .map(|kind| {
                    view! {
                        <section class="panel">
                            <h2>{if kind == "genres" { "Genres" } else { "Formats" }}</h2>
                            <Chart
                                options=Signal::derive(move || breakdown_options(
                                    data
                                        .data
                                        .get()[kind]
                                        .as_array()
                                        .map(Vec::as_slice)
                                        .unwrap_or_default(),
                                ))
                                label=if kind == "genres" {
                                    "Collection by genre; select a slice to filter the library"
                                } else {
                                    "Collection by format; select a slice to filter the library"
                                }
                                filter=if kind == "genres" { "genre" } else { "format" }
                            />
                            <div class="breakdown">
                                {move || {
                                    data
                                        .data
                                        .get()[kind]
                                        .as_array()
                                        .cloned()
                                        .unwrap_or_default()
                                        .into_iter()
                                        .map(|v| {
                                            let name = text(&v, "name");
                                            let query = url::form_urlencoded::Serializer::new(
                                                    String::new(),
                                                )
                                                .append_pair(
                                                    if kind == "genres" { "genre" } else { "format" },
                                                    &name,
                                                )
                                                .finish();
                                            view! {
                                                <A href=format!("/library?{query}")>
                                                    <span>{name}</span>
                                                    <strong>{v["count"].as_i64()}</strong>
                                                </A>
                                            }
                                        })
                                        .collect_view()
                                }}
                            </div>
                        </section>
                    }
                })
                .collect_view()}
        </div>
    }
}

#[component]
fn RecordList(title: String, items: Vec<Value>) -> impl IntoView {
    view! {
        <section class="panel">
            <h2>{title}</h2>
            <div class="ranked-records">
                {items
                    .into_iter()
                    .enumerate()
                    .map(|(index, item)| {
                        view! {
                            <A
                                href=format!("/library/{}", item["instance_id"])
                                attr:class="ranked-record"
                            >
                                <span class="muted">{index + 1}</span>
                                {item["cover_image_url"]
                                    .as_str()
                                    .filter(|s| s.starts_with("https://"))
                                    .map(|src| {
                                        view! {
                                            <img
                                                src=src.to_owned()
                                                alt="Album cover"
                                                loading="lazy"
                                                referrerpolicy="no-referrer"
                                            />
                                        }
                                    })}
                                <span class="record-name">
                                    <strong>{text(&item, "title")}</strong>
                                    <small class="muted">{text(&item, "artist")}</small>
                                </span>
                                <span class="amount">
                                    {money(
                                        item["suggested_value"].as_str(),
                                        item["currency"].as_str(),
                                    )}
                                    <EstimateLabel condition=item["estimate_condition"]
                                        .as_str()
                                        .map(str::to_owned) />
                                </span>
                            </A>
                        }
                    })
                    .collect_view()}
            </div>
        </section>
    }
}

#[component]
fn EstimateLabel(condition: Option<String>) -> impl IntoView {
    condition.map(|condition|view! { <small class="muted estimate-label">{format!("{condition} estimate")}</small> })
}

#[component]
fn HistoryChart(data: Remote) -> impl IntoView {
    let currencies = Memo::new(move |_| {
        data.data.get()["history"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|v| v["currency"].as_str().unwrap_or("unknown").to_owned())
            .collect::<std::collections::BTreeSet<_>>()
    });
    let currency = Signal::derive(move || {
        data.data.get()["display_currency"]
            .as_str()
            .map(str::to_owned)
            .filter(|c| currencies.get().contains(c))
            .or_else(|| currencies.get().first().cloned())
            .unwrap_or_else(|| "unknown".into())
    });
    let options = Signal::derive(move || {
        history_options(
            data.data.get()["history"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default(),
            &currency.get(),
            false,
        )
    });
    view! {
        <section class="panel">
            <h3>"Collection value"</h3>
            <Chart options label="Collection history with zoom and pan" />
            {move || {
                currencies
                    .get()
                    .is_empty()
                    .then_some("No history yet. Sync your collection to create the first snapshot.")
            }}
        </section>
    }
}

#[component]
fn MutationButton(
    action: String,
    label: &'static str,
    #[prop(optional)] refresh: Option<Remote>,
) -> impl IntoView {
    let token = csrf();
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    view! {
        <div>
            <button
                disabled=move || busy.get()
                on:click=move |_| {
                    if busy.get_untracked() {
                        return;
                    }
                    busy.set(true);
                    error.set(None);
                    let action = action.clone();
                    let token = token.clone();
                    leptos::task::spawn_local(async move {
                        match post(&action, vec![("csrf".into(), token)]).await {
                            Ok(()) => {
                                if let Some(data) = refresh {
                                    data.revision.update(|r| *r += 1);
                                }
                            }
                            Err(message) => error.set(Some(message)),
                        };
                        busy.set(false);
                    });
                }
            >
                {move || if busy.get() { "Working…" } else { label }}
            </button>
            <span class="error" role="status">
                {move || error.get()}
            </span>
        </div>
    }
}

#[component]
fn SyncPanel() -> impl IntoView {
    let data = remote(Signal::derive(|| "/api/sync/status".into()));
    let refresh = use_context::<crate::remote::RefreshEpoch>();
    let was_active = StoredValue::new(false);
    Effect::new(move |_| {
        let status = text(&data.data.get()["run"], "status");
        let active = matches!(status.as_str(), "queued" | "running");
        if was_active.get_value()
            && !active
            && let Some(refresh) = refresh
        {
            refresh.0.update(|value| *value += 1);
        }
        was_active.set_value(active);
    });
    #[cfg(target_arch = "wasm32")]
    if let Ok(timer) = set_interval_with_handle(
        move || {
            let status = text(&data.data.get()["run"], "status");
            if matches!(status.as_str(), "queued" | "running") {
                data.revision.update(|r| *r += 1);
            }
        },
        std::time::Duration::from_secs(3),
    ) {
        on_cleanup(move || timer.clear());
    }
    view! {
        <section class="sync-strip panel">
            <div>
                <strong>"Discogs sync"</strong>
                <p class="muted" role="status">
                    {move || {
                        let run = data.data.get()["run"].clone();
                        if run.is_null() {
                            "No sync yet. Import your collection to get started.".into()
                        } else {
                            format!(
                                "{} · {} · {} / {}",
                                text(&run, "status"),
                                text(&run, "phase"),
                                run["processed"].as_i64().unwrap_or(0),
                                run["total"].as_i64().unwrap_or(0),
                            )
                        }
                    }}
                </p>
                <span class="error">
                    {move || data.data.get()["run"]["error"].as_str().map(str::to_owned)}
                </span>
                <div role="status">
                    {move || {
                        data
                            .data
                            .get()["run"]["warnings"]
                            .as_array()
                            .cloned()
                            .unwrap_or_default()
                            .into_iter()
                            .filter_map(|warning| warning.as_str().map(str::to_owned))
                            .map(|warning| view! { <p>{warning}</p> })
                            .collect_view()
                    }}
                </div>
                <Status remote=data />
            </div>
            <MutationButton action="/api/sync".into() label="Sync now" refresh=data />
        </section>
    }
}

#[component]
pub fn Library() -> impl IntoView {
    let location = use_location();
    let query = use_query_map();
    let filters = remote(Signal::derive(|| "/api/library/filters".into()));
    let data = remote(Signal::derive(move || {
        crate::remote::with_query("/api/library", &location.search.get())
    }));
    let grid = RwSignal::new(true);
    let page = Memo::new(move |_| serde_json::from_value::<LibraryPage>(data.data.get()).ok());
    let page_link = move |offset: i64| {
        let mut params: Vec<(String, String)> =
            url::form_urlencoded::parse(location.search.get().trim_start_matches('?').as_bytes())
                .into_owned()
                .filter(|(k, _)| k != "page")
                .collect();
        let current = page.get().map(|p| i64::from(p.page)).unwrap_or(1);
        params.push(("page".into(), (current + offset).max(1).to_string()));
        format!(
            "/library?{}",
            url::form_urlencoded::Serializer::new(String::new())
                .extend_pairs(params)
                .finish()
        )
    };
    view! {
        <div class="page-heading">
            <div>
                <h1>"Your library"</h1>
                <p class="muted">
                    {move || {
                        format!(
                            "{} records match your selection",
                            page.get().map(|p| p.total).unwrap_or(0),
                        )
                    }}
                </p>
            </div>
            <div class="view-toggle" role="group" aria-label="Library view">
                <button aria-pressed=move || !grid.get() on:click=move |_| grid.set(false)>
                    "Table"
                </button>
                <button aria-pressed=move || grid.get() on:click=move |_| grid.set(true)>
                    "Grid"
                </button>
            </div>
        </div>
        <Form method="get" action="/library" attr:class="filters panel">
            <label class="search">
                "Search"
                <input
                    name="q"
                    type="search"
                    placeholder="Artist or title"
                    value=move || query.get().get("q").unwrap_or_default()
                />
            </label>
            <label>
                "Genre"
                <input
                    name="genre"
                    placeholder="Any genre"
                    value=move || query.get().get("genre").unwrap_or_default()
                />
            </label>
            <label>
                "Format"
                <input
                    name="format"
                    placeholder="Vinyl, CD…"
                    value=move || query.get().get("format").unwrap_or_default()
                />
            </label>
            <label>
                "Year"
                <input
                    name="year"
                    type="number"
                    min="0"
                    max="9999"
                    value=move || query.get().get("year").unwrap_or_default()
                />
            </label>
            <label>
                "Decade"
                <input
                    name="decade"
                    type="number"
                    min="0"
                    max="9990"
                    step="10"
                    placeholder="1990"
                    value=move || query.get().get("decade").unwrap_or_default()
                />
            </label>
            <label>
                "Folder"<select name="folder_id">
                    <option
                        value=""
                        selected=move || query.get().get("folder_id").unwrap_or_default().is_empty()
                    >
                        "All folders"
                    </option>
                    {move || {
                        filters
                            .data
                            .get()["metadata"]["folders"]
                            .as_array()
                            .cloned()
                            .unwrap_or_default()
                            .into_iter()
                            .filter(|f| f["id"].as_i64() != Some(0))
                            .map(|f| {
                                let id = f["id"].to_string();
                                let selected = id.clone();
                                view! {
                                    <option
                                        value=id
                                        selected=move || {
                                            query.get().get("folder_id").as_deref()
                                                == Some(selected.as_str())
                                        }
                                    >
                                        {text(&f, "name")}
                                    </option>
                                }
                            })
                            .collect_view()
                    }}
                </select>
            </label>
            <label>
                "Condition"<select name="condition">
                    <option
                        value=""
                        selected=move || query.get().get("condition").unwrap_or_default().is_empty()
                    >
                        "Any condition"
                    </option>
                    {move || {
                        filters
                            .data
                            .get()["filters"]["conditions"]
                            .as_array()
                            .cloned()
                            .unwrap_or_default()
                            .into_iter()
                            .filter_map(|v| v.as_str().map(str::to_owned))
                            .map(|condition| {
                                let selected = condition.clone();
                                let label = condition.clone();
                                view! {
                                    <option
                                        value=condition
                                        selected=move || {
                                            query.get().get("condition").as_deref()
                                                == Some(selected.as_str())
                                        }
                                    >
                                        {label}
                                    </option>
                                }
                            })
                            .collect_view()
                    }}
                </select>
            </label>
            <label>
                "Currency"<select name="currency">
                    <option
                        value=""
                        selected=move || query.get().get("currency").unwrap_or_default().is_empty()
                    >
                        "All currencies"
                    </option>
                    {[
                        "EUR",
                        "USD",
                        "GBP",
                        "CAD",
                        "AUD",
                        "JPY",
                        "CHF",
                        "MXN",
                        "BRL",
                        "NZD",
                        "SEK",
                        "ZAR",
                        "unknown",
                    ]
                        .into_iter()
                        .map(|c| {
                            view! {
                                <option
                                    value=c
                                    selected=move || {
                                        query.get().get("currency").as_deref() == Some(c)
                                    }
                                >
                                    {c}
                                </option>
                            }
                        })
                        .collect_view()}
                </select>
            </label>
            <label>
                "Sort"
                <select name="sort">
                    {[
                        ("added_desc", "Newest additions"),
                        ("added_asc", "Oldest additions"),
                        ("artist", "Artist"),
                        ("title", "Title"),
                        ("year", "Release year"),
                        ("price_desc", "Value: high to low"),
                        ("price_asc", "Value: low to high"),
                    ]
                        .into_iter()
                        .map(|(v, label)| {
                            view! {
                                <option
                                    value=v
                                    selected=move || {
                                        query
                                            .get()
                                            .get("sort")
                                            .unwrap_or_else(|| "added_desc".into()) == v
                                    }
                                >
                                    {label}
                                </option>
                            }
                        })
                        .collect_view()}
                </select>
            </label>
            <button class="primary">"Apply filters"</button>
            <A href="/library">"Clear"</A>
        </Form>
        <Status remote=data />
        {move || {
            page
                .get()
                .map(|p| {
                    if p.items.is_empty() {
                        return view! {
                            <section class="empty panel">
                                <h2>"No records here yet"</h2>
                                <p>"Try different filters, or sync your Discogs collection."</p>
                                <SyncPanel />
                            </section>
                        }
                            .into_any();
                    }
                    if grid.get() {
                        view! {
                            <div class="record-grid">
                                {p
                                    .items
                                    .into_iter()
                                    .map(|item| {
                                        view! {
                                            <A
                                                href=format!("/library/{}", item.instance_id)
                                                attr:class="record-card panel"
                                            >
                                                <div class="cover">
                                                    {item
                                                        .cover_image_url
                                                        .filter(|v| v.starts_with("https://"))
                                                        .map(|src| {
                                                            view! {
                                                                <img
                                                                    src=src
                                                                    alt="Album cover"
                                                                    loading="lazy"
                                                                    referrerpolicy="no-referrer"
                                                                />
                                                            }
                                                        })}
                                                </div>
                                                <h3>{item.title}</h3>
                                                <p>{item.artist}</p>
                                                <small>{item.format}</small>
                                                <strong>
                                                    {money(
                                                        item.suggested_value.as_deref(),
                                                        item.currency.as_deref(),
                                                    )}
                                                </strong>
                                                <EstimateLabel condition=item.estimate_condition />
                                            </A>
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        }
                            .into_any()
                    } else {
                        view! {
                            <div class="table-scroll panel">
                                <table>
                                    <thead>
                                        <tr>
                                            <th>"Artist / title"</th>
                                            <th>"Year"</th>
                                            <th>"Format"</th>
                                            <th>"Condition"</th>
                                            <th>"Value"</th>
                                            <th>"Added"</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {p
                                            .items
                                            .into_iter()
                                            .map(|item| {
                                                view! {
                                                    <tr>
                                                        <td>
                                                            <A href=format!("/library/{}", item.instance_id)>
                                                                <strong>{item.title}</strong>
                                                                <span class="muted">{item.artist}</span>
                                                            </A>
                                                        </td>
                                                        <td>{item.year}</td>
                                                        <td>{item.format}</td>
                                                        <td>
                                                            {item.condition.unwrap_or_else(|| "Not recorded".into())}
                                                        </td>
                                                        <td class="amount">
                                                            {money(
                                                                item.suggested_value.as_deref(),
                                                                item.currency.as_deref(),
                                                            )}<EstimateLabel condition=item.estimate_condition />
                                                        </td>
                                                        <td>
                                                            {item.added_date.chars().take(10).collect::<String>()}
                                                        </td>
                                                    </tr>
                                                }
                                            })
                                            .collect_view()}
                                    </tbody>
                                </table>
                            </div>
                        }
                            .into_any()
                    }
                })
        }}
        <nav class="pagination" aria-label="Library pages">
            {move || {
                page
                    .get()
                    .filter(|p| p.page > 1)
                    .map(|_| view! { <A href=page_link(-1)>"← Previous"</A> })
            }}
            <span>
                {move || {
                    page
                        .get()
                        .map(|p| {
                            format!(
                                "Page {} of {}",
                                p.page,
                                ((p.total + p.page_size as i64 - 1) / p.page_size as i64).max(1),
                            )
                        })
                }}
            </span>
            {move || {
                page
                    .get()
                    .filter(|p| i64::from(p.page) * i64::from(p.page_size) < p.total)
                    .map(|_| view! { <A href=page_link(1)>"Next →"</A> })
            }}
        </nav>
    }
}

#[component]
pub fn RecordDetail() -> impl IntoView {
    let params = use_params_map();
    let data = remote(Signal::derive(move || {
        format!(
            "/api/library/{}",
            params.get().get("id").unwrap_or_default()
        )
    }));
    view! {
        <A href="/library">"← Back to library"</A>
        <Status remote=data />
        {move || {
            let value = data.data.get();
            let item = &value["item"];
            let notes = item["notes"]
                .as_str()
                .map(|raw| {
                    serde_json::from_str::<Vec<Value>>(raw)
                        .map(|notes| {
                            notes.iter().map(|n| text(n, "value")).collect::<Vec<_>>().join("\n")
                        })
                        .unwrap_or_else(|_| raw.into())
                })
                .unwrap_or_default();
            view! {
                <section class="record-detail panel">
                    <div class="cover">
                        {item["cover_image_url"]
                            .as_str()
                            .filter(|v| v.starts_with("https://"))
                            .map(|src| {
                                view! {
                                    <img
                                        src=src.to_string()
                                        alt="Album cover"
                                        referrerpolicy="no-referrer"
                                    />
                                }
                            })}
                    </div>
                    <div>
                        <span class="muted">{text(item, "artist")}</span>
                        <h1>{text(item, "title")}</h1>
                        <p>{text(item, "format")}</p>
                        <p>{text(item, "condition")}</p>
                        <strong class="detail-price">
                            {money(item["suggested_value"].as_str(), item["currency"].as_str())}
                        </strong>
                        <EstimateLabel condition=item["estimate_condition"]
                            .as_str()
                            .map(str::to_owned) />
                        <p>
                            <a
                                href=format!(
                                    "https://www.discogs.com/release/{}",
                                    item["release_id"].as_i64().unwrap_or(0),
                                )
                                target="_blank"
                                rel="noreferrer noopener"
                            >
                                "View release on Discogs ↗"
                            </a>
                        </p>
                        <h2>"Your notes"</h2>
                        <p class="notes">{notes}</p>
                    </div>
                </section>
                <section class="panel">
                    <h2>"Price suggestions by condition"</h2>
                    <div class="breakdown">
                        {value["suggestions"]
                            .as_array()
                            .cloned()
                            .unwrap_or_default()
                            .into_iter()
                            .map(|p| {
                                view! {
                                    <div>
                                        <span>{text(&p, "condition")}</span>
                                        <strong>
                                            {money(p["amount"].as_str(), p["currency"].as_str())}
                                        </strong>
                                    </div>
                                }
                            })
                            .collect_view()}
                    </div>
                </section>
            }
        }}
    }
}

#[component]
pub fn Settings() -> impl IntoView {
    let data = remote(Signal::derive(|| "/api/settings".into()));
    let bootstrap = use_context::<Bootstrap>().unwrap_or_default();
    let username = bootstrap
        .user
        .as_ref()
        .map(|u| text(u, "username"))
        .unwrap_or_default();
    view! {
        <div class="page-heading">
            <h1>"Settings"</h1>
        </div>
        <SyncPanel />
        <Status remote=data />
        {move || {
            (!data.data.get().is_null())
                .then(|| {
                    view! { <PreferencesForm preferences=data.data.get()["preferences"].clone() /> }
                })
        }}
        <section class="panel">
            <h2>"Discogs connection"</h2>
            <p>
                {move || {
                    if data.data.get()["connection"].is_null() {
                        "Disconnected. Your library is retained; synchronization is paused."
                    } else {
                        "Connected. Sync runs with your Discogs credentials."
                    }
                }}
            </p>
            <div class="actions">
                <form method="post" action="/auth/reconnect">
                    <input name="csrf" type="hidden" value=csrf() />
                    <button>"Reconnect Discogs"</button>
                </form>
                <form method="post" action="/auth/disconnect">
                    <input name="csrf" type="hidden" value=csrf() />
                    <button>"Disconnect"</button>
                </form>
            </div>
            <p class="muted">
                "Disconnecting removes stored credentials and stops syncs. Revoke the application in Discogs too if you want to remove its upstream authorization."
            </p>
        </section>
        <section class="panel">
            <h2>"Sessions"</h2>
            <p>
                "Sessions expire 90 days after sign-in. Signing out does not disconnect Discogs or stop scheduled sync."
            </p>
            <form method="post" action="/auth/revoke-sessions">
                <input name="csrf" type="hidden" value=csrf() />
                <button>"Sign out everywhere"</button>
            </form>
        </section>
        <section class="panel danger">
            <h2>"Delete account"</h2>
            <p>
                "Permanently delete your account, collection, history, settings, and stored credentials. This cannot be undone. The last administrator cannot be deleted."
            </p>
            <form method="post" action="/auth/delete-account">
                <input name="csrf" type="hidden" value=csrf() />
                <label>
                    {format!("Type {username} to confirm")}
                    <input name="confirm" required autocomplete="off" />
                </label>
                <button class="danger-button">"Delete my account"</button>
            </form>
        </section>
    }
}

#[component]
fn PreferencesForm(preferences: Value) -> impl IntoView {
    let sync = preferences["sync_interval_hours"].as_i64().unwrap_or(24);
    let prices = preferences["price_refresh_hours"].as_i64().unwrap_or(24);
    let schedule = if sync == 0 {
        format!("Automatic sync is off. Cached prices refresh after {prices} hours.")
    } else {
        format!("Sync runs every {sync} hours. Cached prices refresh after {prices} hours.")
    };
    let currency = RwSignal::new(text(&preferences, "display_currency"));
    let message = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let token = csrf();
    view! {
        <section class="panel">
            <h2>"Preferences"</h2>
            <form
                class="settings-form"
                method="post"
                action="/api/settings"
                on:submit=move |event| {
                    event.prevent_default();
                    if busy.get_untracked() {
                        return;
                    }
                    busy.set(true);
                    message.set(String::new());
                    let fields = vec![
                        ("csrf".into(), token.clone()),
                        ("display_currency".into(), currency.get_untracked()),
                    ];
                    leptos::task::spawn_local(async move {
                        message
                            .set(
                                match post("/api/settings", fields).await {
                                    Ok(()) => "Preferences saved.".into(),
                                    Err(e) => e,
                                },
                            );
                        busy.set(false);
                    });
                }
            >
                <input name="csrf" type="hidden" value=csrf() />
                <p class="muted">
                    {schedule}
                    " An administrator manages these intervals."
                </p>
                <label>
                    "Preferred display currency"
                    <select
                        name="display_currency"
                        on:change=move |ev| currency.set(event_target_value(&ev))
                    >
                        <option value="" selected=move || currency.get().is_empty()>
                            "Automatic"
                        </option>
                        {[
                            "EUR",
                            "USD",
                            "GBP",
                            "CAD",
                            "AUD",
                            "JPY",
                            "CHF",
                            "MXN",
                            "BRL",
                            "NZD",
                            "SEK",
                            "ZAR",
                        ]
                            .into_iter()
                            .map(|c| {
                                view! {
                                    <option value=c selected=move || currency.get() == c>
                                        {c}
                                    </option>
                                }
                            })
                            .collect_view()}
                    </select>
                    <small>"Selects the initial chart currency. Does not convert amounts."</small>
                </label>
                <button class="primary" disabled=move || busy.get()>
                    "Save preferences"
                </button>
                <p role="status">{move || message.get()}</p>
            </form>
        </section>
    }
}

#[component]
pub fn Admin() -> impl IntoView {
    let data = remote(Signal::derive(|| "/api/admin/users".into()));
    let me = use_context::<Bootstrap>()
        .unwrap_or_default()
        .user
        .and_then(|u| u["id"].as_i64());
    let notice = RwSignal::new(String::new());
    let settings = Memo::new(move |_| data.data.get()["settings"].clone());
    // Keyed by row content so refetches keep unsaved input in unchanged rows.
    let users = Memo::new(move |_| {
        data.data.get()["users"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|user| (user.to_string(), user))
            .collect::<Vec<_>>()
    });
    view! {
        <div class="page-heading">
            <h1>"Administration"</h1>
        </div>
        <Status remote=data />
        <p id="admin-notice" class="muted" role="status">
            {move || notice.get()}
        </p>
        {move || {
            let values = settings.get();
            (!values.is_null())
                .then(|| {
                    view! {
                        <section class="panel">
                            <h2>"Sync defaults"</h2>
                            <IntervalsForm
                                action="/admin/settings".into()
                                values=values.clone()
                                refresh=data
                                notice=notice
                                saved="Defaults saved.".into()
                            />
                        </section>
                    }
                })
        }}
        <div class="approval-list">
            <For
                each=move || users.get()
                key=|(key, _)| key.clone()
                children=move |(_, user)| {
                    view! { <UserCard user=user me=me defaults=settings.into() refresh=data notice=notice /> }
                }
            />
        </div>
    }
}

#[component]
fn UserCard(
    user: Value,
    me: Option<i64>,
    defaults: Signal<Value>,
    refresh: Remote,
    notice: RwSignal<String>,
) -> impl IntoView {
    let id = user["id"].as_i64().unwrap_or(0);
    let status = text(&user, "status");
    let username = text(&user, "username");
    let own = me == Some(id);
    let sync = &user["last_sync"];
    let last_sync = if sync.is_null() {
        "never".into()
    } else {
        format!("{} {}", text(sync, "status"), day(&sync["at"]))
    };
    let summary = format!(
        "{} · {status} · Discogs ID {}",
        text(&user, "role"),
        user["discogs_id"].as_i64().unwrap_or(0),
    );
    let activity = format!(
        "Joined {} · Last login {} · {} items · Last sync {last_sync}",
        day(&user["created_at"]),
        day(&user["last_login_at"]),
        user["items"].as_i64().unwrap_or(0),
    );
    let actions: &[(&str, &'static str)] = match status.as_str() {
        "pending" => &[("approve", "Approve"), ("reject", "Reject")],
        "rejected" => &[("approve", "Approve")],
        "disabled" => &[("enable", "Enable")],
        _ => &[],
    };
    let disable = !own && status != "disabled";
    let saved = format!("Overrides saved for {username}.");
    let heading = username.clone();
    let confirm_name = username.clone();
    view! {
        <section class="panel admin-user" data-user=username>
            <div class="approval">
                <div>
                    <h2>{heading}</h2>
                    <span class="muted">{summary}</span>
                </div>
                <div class="actions">
                    {actions
                        .iter()
                        .map(|(path, label)| {
                            view! {
                                <MutationButton
                                    action=format!("/admin/users/{id}/{path}")
                                    label=*label
                                    refresh=refresh
                                />
                            }
                        })
                        .collect_view()}
                    {disable
                        .then(|| {
                            view! {
                                <MutationButton
                                    action=format!("/admin/users/{id}/disable")
                                    label="Disable"
                                    refresh=refresh
                                />
                            }
                        })}
                </div>
            </div>
            <p class="muted">{activity}</p>
            <IntervalsForm
                action=format!("/admin/users/{id}/overrides")
                values=user.clone()
                defaults=defaults
                refresh=refresh
                notice=notice
                saved=saved
            />
            {(!own).then(|| view! { <DeleteUserForm id=id username=confirm_name refresh=refresh /> })}
        </section>
    }
}

fn day(value: &Value) -> String {
    value
        .as_str()
        .and_then(|v| v.get(..10))
        .unwrap_or("never")
        .into()
}

#[component]
fn IntervalsForm(
    action: String,
    values: Value,
    #[prop(optional, into)] defaults: Option<Signal<Value>>,
    refresh: Remote,
    notice: RwSignal<String>,
    saved: String,
) -> impl IntoView {
    let field = |key: &str| {
        values[key]
            .as_i64()
            .map(|v| v.to_string())
            .unwrap_or_default()
    };
    let sync = RwSignal::new(field("sync_interval_hours"));
    let prices = RwSignal::new(field("price_refresh_hours"));
    let overrides = defaults.is_some();
    let placeholder = move |key: &'static str| {
        move || {
            defaults
                .and_then(|d| d.get()[key].as_i64())
                .map(|v| format!("Default ({v})"))
                .unwrap_or_default()
        }
    };
    let sync_placeholder = placeholder("sync_interval_hours");
    let prices_placeholder = placeholder("price_refresh_hours");
    let message = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let token = csrf();
    let target = action.clone();
    view! {
        <form
            class="settings-form"
            method="post"
            action=action
            on:submit=move |event| {
                event.prevent_default();
                if busy.get_untracked() {
                    return;
                }
                busy.set(true);
                message.set(String::new());
                notice.set(String::new());
                let fields = vec![
                    ("csrf".into(), token.clone()),
                    ("sync_interval_hours".into(), sync.get_untracked()),
                    ("price_refresh_hours".into(), prices.get_untracked()),
                ];
                let target = target.clone();
                let saved = saved.clone();
                leptos::task::spawn_local(async move {
                    match post(&target, fields).await {
                        Ok(()) => {
                            notice.set(saved);
                            refresh.revision.update(|r| *r += 1);
                        }
                        Err(e) => message.set(e),
                    }
                    busy.set(false);
                });
            }
        >
            <input name="csrf" type="hidden" value=csrf() />
            <label>
                "Sync interval (hours)"
                <input
                    name="sync_interval_hours"
                    type="number"
                    min="0"
                    max="720"
                    required=!overrides
                    placeholder=sync_placeholder
                    value=move || sync.get()
                    prop:value=move || sync.get()
                    on:input=move |ev| sync.set(event_target_value(&ev))
                /><small>"24 is daily. Set 0 for manual sync only."</small>
            </label>
            <label>
                "Refresh cached prices after (hours)"
                <input
                    name="price_refresh_hours"
                    type="number"
                    min="1"
                    max="720"
                    required=!overrides
                    placeholder=prices_placeholder
                    value=move || prices.get()
                    prop:value=move || prices.get()
                    on:input=move |ev| prices.set(event_target_value(&ev))
                /> {overrides.then_some(view! { <small>"Leave empty to use the default."</small> })}
            </label>
            <button disabled=move || {
                busy.get()
            }>{if overrides { "Save overrides" } else { "Save defaults" }}</button>
            <p role="status">{move || message.get()}</p>
        </form>
    }
}

#[component]
fn DeleteUserForm(id: i64, username: String, refresh: Remote) -> impl IntoView {
    let confirm = RwSignal::new(String::new());
    let error = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let token = csrf();
    let action = format!("/admin/users/{id}/delete");
    let target = action.clone();
    view! {
        <div class="danger">
            <form
                method="post"
                action=action
                on:submit=move |event| {
                    event.prevent_default();
                    if busy.get_untracked() {
                        return;
                    }
                    busy.set(true);
                    error.set(String::new());
                    let fields = vec![
                        ("csrf".into(), token.clone()),
                        ("confirm".into(), confirm.get_untracked()),
                    ];
                    let target = target.clone();
                    leptos::task::spawn_local(async move {
                        match post(&target, fields).await {
                            Ok(()) => refresh.revision.update(|r| *r += 1),
                            Err(e) => error.set(e),
                        }
                        busy.set(false);
                    });
                }
            >
                <input name="csrf" type="hidden" value=csrf() />
                <label>
                    {format!("Type {username} to delete this account and all its data")}
                    <input
                        name="confirm"
                        required
                        autocomplete="off"
                        prop:value=move || confirm.get()
                        on:input=move |ev| confirm.set(event_target_value(&ev))
                    />
                </label>
                <button class="danger-button" disabled=move || busy.get()>
                    "Delete user"
                </button>
                <span class="error" role="status">
                    {move || error.get()}
                </span>
            </form>
        </div>
    }
}
