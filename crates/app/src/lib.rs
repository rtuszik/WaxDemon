#![recursion_limit = "256"]

mod charts;
mod pages;
mod remote;

use leptos::prelude::*;
use leptos_router::{
    components::{A, Route, Router, Routes},
    path,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Bootstrap {
    pub user: Option<Value>,
    pub csrf: String,
    pub data: BTreeMap<String, Value>,
}

#[component]
pub fn App() -> impl IntoView {
    leptos_meta::provide_meta_context();
    provide_context(remote::RefreshEpoch(RwSignal::new(0)));
    let bootstrap = use_context::<Bootstrap>().unwrap_or_default();
    let approved = bootstrap
        .user
        .as_ref()
        .is_some_and(|u| u["status"] == "approved");
    let admin = approved
        && bootstrap
            .user
            .as_ref()
            .is_some_and(|u| u["role"] == "admin");
    let username = bootstrap
        .user
        .as_ref()
        .and_then(|u| u["username"].as_str())
        .unwrap_or("")
        .to_string();
    let csrf = bootstrap.csrf.clone();
    view! {
        <Router>
            <header class="topbar">
                <A href="/" attr:class="brand">"WaxDemon"</A>
                {approved.then(||view!{<nav aria-label="Main navigation"><A href="/">"Overview"</A><A href="/library">"Library"</A><A href="/settings">"Settings"</A>{admin.then(||view!{<A href="/admin/users">"Approvals"</A>})}</nav>})}
                {bootstrap.user.is_some().then(||view!{<div class="account-menu"><span>{username}</span><form method="post" action="/auth/logout"><input type="hidden" name="csrf" value=csrf/><button class="quiet">"Sign out"</button></form></div>})}
            </header>
            <main id="main-content">
                <Routes fallback=||view!{<h1>"Page not found"</h1><A href="/">"Return to overview"</A>}>
                    <Route path=path!("") view=pages::Overview/>
                    <Route path=path!("auth/login") view=pages::Login/>
                    <Route path=path!("library") view=pages::Library/>
                    <Route path=path!("library/:id") view=pages::RecordDetail/>
                    <Route path=path!("settings") view=pages::Settings/>
                    <Route path=path!("admin/users") view=pages::Approvals/>
                </Routes>
            </main>
        </Router>
    }
}

#[cfg(feature = "ssr")]
pub fn shell(options: LeptosOptions, bootstrap: Bootstrap) -> impl IntoView {
    let json = serde_json::to_string(&bootstrap)
        .unwrap()
        .replace('<', "\\u003c")
        .replace('&', "\\u0026");
    view! {
        <!DOCTYPE html>
        <html lang="en"><head><meta charset="utf-8"/><meta name="viewport" content="width=device-width,initial-scale=1"/>
            <title>"WaxDemon"</title>
            <link rel="stylesheet" href="/pkg/waxdemon.css"/>
            <link rel="icon" href="/assets/favicon.ico"/>
            <link rel="apple-touch-icon" href="/assets/apple-touch-icon.png"/>
            <link rel="manifest" href="/assets/site.webmanifest"/>
            <script defer src="/assets/echarts-6.1.0.min.js"></script>
            <HydrationScripts options/>
            <leptos_meta::MetaTags/>
        </head><body><App/><script id="wax-bootstrap" type="application/json" inner_html=json></script></body></html>
    }
}

#[cfg(all(feature = "hydrate", target_arch = "wasm32"))]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    let bootstrap = leptos::prelude::document()
        .get_element_by_id("wax-bootstrap")
        .and_then(|e| e.text_content())
        .and_then(|s| serde_json::from_str::<Bootstrap>(&s).ok())
        .unwrap_or_default();
    leptos::mount::hydrate_body(move || {
        provide_context(crate::remote::HydrationRequests(StoredValue::new(
            bootstrap.data.keys().cloned().collect(),
        )));
        provide_context(bootstrap.clone());
        view! {<App/>}
    });
}
