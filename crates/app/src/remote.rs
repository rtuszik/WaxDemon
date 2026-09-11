use crate::Bootstrap;
use leptos::prelude::*;
use serde_json::Value;

#[derive(Clone, Copy)]
pub struct RefreshEpoch(pub RwSignal<u64>);

pub fn with_query(path: &str, search: &str) -> String {
    let search = search.trim_start_matches('?');
    if search.is_empty() {
        path.into()
    } else {
        format!("{path}?{search}")
    }
}

#[derive(Clone, Copy)]
pub struct Remote {
    pub data: RwSignal<Value>,
    pub loading: RwSignal<bool>,
    pub error: RwSignal<Option<String>>,
    pub revision: RwSignal<u64>,
}

pub fn remote(endpoint: Signal<String>) -> Remote {
    let bootstrap = use_context::<Bootstrap>().unwrap_or_default();
    let initial = bootstrap
        .data
        .get(&endpoint.get_untracked())
        .cloned()
        .unwrap_or(Value::Null);
    let remote = Remote {
        loading: RwSignal::new(initial.is_null()),
        data: RwSignal::new(initial),
        error: RwSignal::new(None),
        revision: RwSignal::new(0),
    };
    #[cfg(target_arch = "wasm32")]
    {
        let generation = RwSignal::new(0u64);
        let refresh = use_context::<RefreshEpoch>();
        Effect::new(move |_| {
            let url = endpoint.get();
            remote.revision.get();
            if let Some(refresh) = refresh {
                refresh.0.get();
            }
            generation.update(|value| *value += 1);
            let request_id = generation.get_untracked();
            remote.loading.set(true);
            remote.error.set(None);
            leptos::task::spawn_local(async move {
                let result = get(&url).await;
                if generation.try_get_untracked() != Some(request_id)
                    || endpoint.try_get_untracked().as_deref() != Some(url.as_str())
                {
                    return;
                }
                remote.loading.set(false);
                match result {
                    Ok(value) => remote.data.set(value),
                    Err(error) => remote.error.set(Some(error)),
                }
            });
        });
    }
    remote
}

#[cfg(target_arch = "wasm32")]
async fn get(url: &str) -> Result<Value, String> {
    let response = gloo_net::http::Request::get(url)
        .send()
        .await
        .map_err(|_| "Could not reach the server. Try again.".to_string())?;
    match response.status() {
        200 => response
            .json()
            .await
            .map_err(|_| "The server returned an invalid response.".into()),
        401 => Err("Your session has expired. Sign in again.".into()),
        403 => Err("You do not have access to this page.".into()),
        404 => Err("This record is no longer in your library.".into()),
        _ => Err("Could not load this page. Try again.".into()),
    }
}

pub async fn post(url: &str, fields: Vec<(String, String)>) -> Result<(), String> {
    #[cfg(target_arch = "wasm32")]
    {
        let body = url::form_urlencoded::Serializer::new(String::new())
            .extend_pairs(fields)
            .finish();
        let response = gloo_net::http::Request::post(url)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .map_err(|_| "Invalid request.".to_string())?
            .send()
            .await
            .map_err(|_| "Could not reach the server.".to_string())?;
        if response.ok() {
            Ok(())
        } else {
            Err(response
                .text()
                .await
                .unwrap_or_else(|_| "Request failed.".into()))
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (url, fields);
        Err("This action requires a browser.".into())
    }
}

#[component]
pub fn Status(remote: Remote) -> impl IntoView {
    view! {
        <div class="request-status" role="status" aria-live="polite">
            {move ||remote.loading.get().then_some("Loading…")}
            {move ||remote.error.get().map(|message|view!{<div class="error"><span>{message}</span><button on:click=move |_|remote.revision.update(|r|*r+=1)>"Try again"</button><a href="/auth/login">"Sign in"</a></div>})}
        </div>
    }
}

pub fn text(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or("").into()
}

pub fn money(amount: Option<&str>, currency: Option<&str>) -> String {
    match amount {
        None => "—".into(),
        Some(value) => {
            let display = value
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite())
                .map(|v| format!("{v:.2}"))
                .unwrap_or_else(|| value.into());
            format!("{} {}", currency.unwrap_or("Unknown currency"), display)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn query_urls_match_server_bootstrap_and_browser_navigation() {
        assert_eq!(
            with_query("/api/library", "q=Jazz%20%26%20Blues"),
            "/api/library?q=Jazz%20%26%20Blues"
        );
        assert_eq!(
            with_query("/api/library", "?q=First"),
            "/api/library?q=First"
        );
        assert_eq!(with_query("/api/dashboard", ""), "/api/dashboard");
    }
    #[test]
    fn values_keep_currency_and_distinguish_missing_from_zero() {
        assert_eq!(money(Some("0"), Some("EUR")), "EUR 0.00");
        assert_eq!(money(Some("12.345"), None), "Unknown currency 12.35");
        assert_eq!(
            money(Some("109.0591666666666667"), Some("EUR")),
            "EUR 109.06"
        );
        assert_eq!(money(None, Some("USD")), "—");
    }
}
