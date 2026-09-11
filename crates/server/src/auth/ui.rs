use super::{AuthError, AuthSession, AuthState, api, routes};
use axum::{
    Json,
    extract::{Path, Query, Request, State},
    http::header,
    response::{IntoResponse, Redirect, Response},
};
use leptos::{nonce::Nonce, prelude::*};
use serde_json::{Value, json};
use waxdemon_app::Bootstrap;

pub(super) async fn pending_users(
    State(state): State<AuthState>,
    auth: AuthSession,
) -> Result<Json<Value>, AuthError> {
    routes::admin(&auth)?;
    let users:Vec<super::backend::User>=sqlx::query_as("SELECT id,discogs_id,username,role,status,session_revocation::text FROM users WHERE status='pending' ORDER BY created_at,id LIMIT 100").fetch_all(&state.pool).await?;
    Ok(Json(json!({"users":users})))
}

pub(super) async fn render(
    State(state): State<AuthState>,
    auth: AuthSession,
    request: Request,
) -> Result<Response, AuthError> {
    let path = request.uri().path();
    if auth.user.is_none() && path != "/auth/login" {
        return Ok(Redirect::to("/auth/login").into_response());
    }
    if auth.user.is_some() && path == "/auth/login" {
        return Ok(Redirect::to("/").into_response());
    }
    let is_approved = auth.user.as_ref().is_some_and(|u| u.status == "approved");
    if path == "/admin/users" {
        routes::admin(&auth)?;
    }
    if !is_approved && !matches!(path, "/" | "/auth/login") {
        return Ok(Redirect::to("/").into_response());
    }
    let mut bootstrap = Bootstrap {
        user: auth.user.as_ref().map(|u| json!(u)),
        csrf: routes::csrf(&auth.session).await?,
        ..Default::default()
    };
    let search = request
        .uri()
        .query()
        .map(|v| format!("?{v}"))
        .unwrap_or_default();
    if is_approved {
        if path == "/library" {
            bootstrap.data.insert(
                "/api/library/filters".into(),
                api::filters(State(state.clone()), auth.clone()).await?.0,
            );
        }
        bootstrap.data.insert(
            "/api/sync/status".into(),
            routes::sync_status(State(state.clone()), auth.clone())
                .await?
                .0,
        );
        let (endpoint, data) = match path {
            "/" => (
                format!("/api/dashboard{search}"),
                api::dashboard(
                    State(state.clone()),
                    auth.clone(),
                    Query::try_from_uri(request.uri()).map_err(|_| AuthError::InvalidInput)?,
                )
                .await?
                .0,
            ),
            "/library" => (
                format!("/api/library{search}"),
                json!(
                    api::library(
                        State(state.clone()),
                        auth.clone(),
                        Query::try_from_uri(request.uri()).map_err(|_| AuthError::InvalidInput)?
                    )
                    .await?
                    .0
                ),
            ),
            "/settings" => (
                "/api/settings".into(),
                api::settings(State(state.clone()), auth.clone()).await?.0,
            ),
            "/admin/users" => (
                "/api/admin/users".into(),
                pending_users(State(state.clone()), auth.clone()).await?.0,
            ),
            _ if path.starts_with("/library/") => {
                let id = path
                    .trim_start_matches("/library/")
                    .parse()
                    .map_err(|_| AuthError::NotFound)?;
                (
                    format!("/api/library/{id}"),
                    api::item(State(state.clone()), auth.clone(), Path(id))
                        .await?
                        .0,
                )
            }
            _ => return Err(AuthError::NotFound),
        };
        bootstrap.data.insert(endpoint, data);
    }
    let options = state.leptos.clone();
    let nonce = Nonce::new();
    let policy = format!(
        "default-src 'self'; script-src 'self' 'nonce-{nonce}' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' https://i.discogs.com https://img.discogs.com https://api-img.discogs.com data:; connect-src 'self'; form-action 'self' https://www.discogs.com/oauth/authorize; frame-ancestors 'none'; base-uri 'none'"
    );
    let context = bootstrap.clone();
    let handler = leptos_axum::render_app_to_stream_with_context(
        move || {
            provide_context(context.clone());
            provide_context(nonce.clone());
        },
        move || waxdemon_app::shell(options.clone(), bootstrap.clone()),
    );
    let mut response = handler(request).await;
    response
        .headers_mut()
        .insert(header::REFERRER_POLICY, "same-origin".parse().unwrap());
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        policy.parse().map_err(|_| AuthError::Internal)?,
    );
    Ok(response)
}
