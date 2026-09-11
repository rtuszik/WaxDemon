use super::{
    AuthError, AuthSession, AuthState, SESSION_DAYS,
    backend::{LoginCredentials, User},
    store::{PgSessionStore, hash},
};
use axum::{
    Form, Json, Router,
    extract::{DefaultBodyLimit, Path, Query, Request, State},
    http::{HeaderMap, header},
    middleware::{self, Next},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use axum_login::AuthManagerLayerBuilder;
use chacha20poly1305::aead::Generate;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use subtle::ConstantTimeEq;
use time::{Duration, OffsetDateTime};
use tower_sessions::{Expiry, Session, SessionManagerLayer, cookie::SameSite};
use waxdemon_db::connections::EncryptedConnection;
use waxdemon_discogs::oauth::RequestToken;

const CSRF_KEY: &str = "csrf";
const ATTEMPT_KEY: &str = "oauth_attempt";
const RECONNECT_KEY: &str = "oauth_reconnect_user";
const DEADLINE_KEY: &str = "auth_expires";

pub fn router(state: AuthState) -> Router {
    let _ = any_spawner::Executor::init_tokio();
    let store = PgSessionStore::new(state.pool.clone());
    let sessions = SessionManagerLayer::new(store)
        .with_name(if state.secure {
            "__Host-waxdemon.sid"
        } else {
            "waxdemon.sid"
        })
        .with_secure(state.secure)
        .with_http_only(true)
        .with_same_site(SameSite::Lax)
        .with_path("/")
        .with_expiry(Expiry::OnInactivity(Duration::minutes(15)));
    let auth = AuthManagerLayerBuilder::new(state.clone(), sessions).build();
    Router::new()
        .route("/health/live", get(super::health::live))
        .route("/health/ready", get(super::health::ready))
        .merge(super::api::router())
        .route("/", get(super::ui::render))
        .route("/library", get(super::ui::render))
        .route("/library/{id}", get(super::ui::render))
        .route("/settings", get(super::ui::render))
        .route("/auth/login", get(super::ui::render).post(begin_login))
        .route("/auth/callback", get(callback))
        .route("/auth/me", get(me))
        .route("/auth/logout", post(logout))
        .route("/auth/reconnect", post(begin_reconnect))
        .route("/auth/disconnect", post(disconnect))
        .route("/auth/delete-account", post(delete_account))
        .route("/auth/revoke-sessions", post(revoke_sessions))
        .route("/api/sync", post(queue_sync))
        .route("/api/sync/status", get(sync_status))
        .route("/admin/users", get(super::ui::render))
        .route("/api/admin/users", get(super::ui::pending_users))
        .nest_service(
            "/pkg",
            tower_http::services::ServeDir::new(format!("{}/pkg", state.leptos.site_root)),
        )
        .nest_service(
            "/assets",
            tower_http::services::ServeDir::new(format!("{}/assets", state.leptos.site_root)),
        )
        .route_service(
            "/favicon.ico",
            tower_http::services::ServeFile::new(format!(
                "{}/assets/favicon.ico",
                state.leptos.site_root
            )),
        )
        .route("/admin/users/{id}/approve", post(approve))
        .route("/admin/users/{id}/reject", post(reject))
        .layer(DefaultBodyLimit::max(8192))
        .layer(middleware::from_fn(enforce_deadline))
        .layer(auth)
        .layer(middleware::from_fn(security_headers))
        .with_state(state)
}

async fn security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    headers
        .entry(header::REFERRER_POLICY)
        .or_insert("no-referrer".parse().unwrap());
    headers.insert(header::X_CONTENT_TYPE_OPTIONS, "nosniff".parse().unwrap());
    headers.entry(header::CONTENT_SECURITY_POLICY).or_insert(
        "default-src 'none'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'"
            .parse()
            .unwrap(),
    );
    response
}

async fn enforce_deadline(
    auth: AuthSession,
    request: Request,
    next: Next,
) -> Result<Response, AuthError> {
    if auth.user.is_some() {
        let deadline = auth
            .session
            .get::<i64>(DEADLINE_KEY)
            .await?
            .and_then(|v| OffsetDateTime::from_unix_timestamp(v).ok());
        match deadline {
            Some(deadline) if deadline > OffsetDateTime::now_utc() => {
                auth.session.set_expiry(Some(Expiry::AtDateTime(deadline)))
            }
            _ => {
                auth.session.flush().await?;
                return Err(AuthError::Unauthorized);
            }
        }
    }
    Ok(next.run(request).await)
}

fn random_token() -> Result<String, AuthError> {
    Ok(hex::encode(
        <[u8; 32]>::try_generate().map_err(|_| AuthError::Internal)?,
    ))
}

pub(super) async fn csrf(session: &Session) -> Result<String, AuthError> {
    if let Some(token) = session.get(CSRF_KEY).await? {
        return Ok(token);
    }
    let token = random_token()?;
    session.insert(CSRF_KEY, &token).await?;
    Ok(token)
}

#[derive(Deserialize)]
struct Mutation {
    csrf: String,
}

pub(super) async fn check_csrf(
    state: &AuthState,
    session: &Session,
    headers: &HeaderMap,
    supplied: &str,
) -> Result<(), AuthError> {
    let origin = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok());
    let expected: Option<String> = session.get(CSRF_KEY).await?;
    if origin != Some(state.origin.as_str())
        || !expected.is_some_and(|v| bool::from(v.as_bytes().ct_eq(supplied.as_bytes())))
    {
        return Err(AuthError::Forbidden);
    }
    Ok(())
}

async fn begin_login(
    State(state): State<AuthState>,
    auth: AuthSession,
    headers: HeaderMap,
    Form(input): Form<Mutation>,
) -> Result<Redirect, AuthError> {
    check_csrf(&state, &auth.session, &headers, &input.csrf).await?;
    if auth.user.is_some() {
        return Err(AuthError::Conflict);
    }
    auth.session.remove::<i64>(RECONNECT_KEY).await?;
    start_attempt(state, auth, None).await
}

async fn begin_reconnect(
    State(state): State<AuthState>,
    auth: AuthSession,
    headers: HeaderMap,
    Form(input): Form<Mutation>,
) -> Result<Redirect, AuthError> {
    check_csrf(&state, &auth.session, &headers, &input.csrf).await?;
    let id = user(&auth)?.id;
    auth.session.insert(RECONNECT_KEY, id).await?;
    start_attempt(state, auth, Some(id)).await
}

async fn start_attempt(
    state: AuthState,
    auth: AuthSession,
    owner: Option<i64>,
) -> Result<Redirect, AuthError> {
    if let Some(old) = auth.session.remove::<String>(ATTEMPT_KEY).await? {
        sqlx::query("DELETE FROM oauth_attempts WHERE browser_hash = $1")
            .bind(hash(&old))
            .execute(&state.pool)
            .await?;
    }
    let request = state
        .oauth
        .request_token(&state.callback)
        .await
        .map_err(|_| AuthError::Provider)?;
    let binding = random_token()?;
    let browser_hash = hash(&binding);
    let encrypted = state
        .vault
        .encrypt_attempt(&hex::encode(&browser_hash), request.credentials())
        .map_err(|_| AuthError::Internal)?;
    sqlx::query("INSERT INTO oauth_attempts (browser_hash, token_hash, key_id, nonce, ciphertext, expires_at,user_id)
        VALUES ($1, $2, $3, $4, $5, now() + interval '15 minutes',$6)")
        .bind(browser_hash).bind(hash(request.credentials().token().expose_secret()))
        .bind(encrypted.key_id).bind(encrypted.nonce).bind(encrypted.ciphertext).bind(owner).execute(&state.pool).await?;
    auth.session.insert(ATTEMPT_KEY, binding).await?;
    auth.session.save().await?;
    Ok(Redirect::to(request.authorization_url().as_str()))
}

#[derive(Deserialize)]
struct Callback {
    oauth_token: Option<SecretString>,
    oauth_verifier: Option<SecretString>,
    denied: Option<SecretString>,
}

async fn callback(
    State(state): State<AuthState>,
    mut auth: AuthSession,
    Query(input): Query<Callback>,
) -> Result<Redirect, AuthError> {
    let expected_user: Option<i64> = auth.session.get(RECONNECT_KEY).await?;
    if auth.user.as_ref().map(|u| u.id) != expected_user {
        return Err(AuthError::Conflict);
    }
    let token = input
        .oauth_token
        .as_ref()
        .or(input.denied.as_ref())
        .ok_or(AuthError::Attempt)?;
    let binding: String = auth
        .session
        .get(ATTEMPT_KEY)
        .await?
        .ok_or(AuthError::Attempt)?;
    let browser_hash = hash(&binding);
    let encrypted: EncryptedConnection = sqlx::query_as("DELETE FROM oauth_attempts WHERE browser_hash = $1 AND token_hash = $2 AND expires_at > now() AND user_id IS NOT DISTINCT FROM $3 RETURNING key_id, nonce, ciphertext")
        .bind(&browser_hash).bind(hash(token.expose_secret())).bind(expected_user).fetch_optional(&state.pool).await?.ok_or(AuthError::Attempt)?;
    auth.session.remove::<String>(ATTEMPT_KEY).await?;
    auth.session.remove::<i64>(RECONNECT_KEY).await?;
    auth.session.save().await?;
    if input.denied.is_some() {
        return Err(AuthError::Forbidden);
    }
    let verifier = input.oauth_verifier.ok_or(AuthError::Attempt)?;
    let credentials = state
        .vault
        .decrypt_attempt(&hex::encode(browser_hash), &encrypted)
        .map_err(|_| AuthError::Attempt)?;
    let access = state
        .oauth
        .exchange(&RequestToken::from_credentials(credentials), &verifier)
        .await
        .map_err(|_| AuthError::Provider)?;
    let user = auth
        .authenticate(LoginCredentials {
            access,
            expected_user,
        })
        .await
        .map_err(|_| AuthError::Provider)?
        .ok_or(AuthError::Forbidden)?;
    auth.session.flush().await?;
    auth.session.cycle_id().await?;
    let deadline = OffsetDateTime::now_utc() + Duration::days(SESSION_DAYS);
    auth.session.set_expiry(Some(Expiry::AtDateTime(deadline)));
    auth.session
        .insert(DEADLINE_KEY, deadline.unix_timestamp())
        .await?;
    auth.session.insert(CSRF_KEY, random_token()?).await?;
    auth.login(&user).await.map_err(|_| AuthError::Internal)?;
    auth.session.save().await?;
    Ok(Redirect::to("/"))
}

fn user(auth: &AuthSession) -> Result<&User, AuthError> {
    auth.user.as_ref().ok_or(AuthError::Unauthorized)
}

async fn disconnect(
    State(state): State<AuthState>,
    auth: AuthSession,
    headers: HeaderMap,
    Form(input): Form<Mutation>,
) -> Result<Redirect, AuthError> {
    check_csrf(&state, &auth.session, &headers, &input.csrf).await?;
    let id = user(&auth)?.id;
    let mut tx = state.pool.begin().await?;
    sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM discogs_connections WHERE user_id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM oauth_attempts WHERE user_id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE user_sync_runs SET status='cancelled',phase='cancelled',error='Discogs disconnected',finished_at=now() WHERE user_id=$1 AND status IN ('queued','running')")
        .bind(id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(Redirect::to("/settings"))
}

#[derive(Deserialize)]
struct DeleteAccount {
    csrf: String,
    confirm: String,
}

async fn delete_account(
    State(state): State<AuthState>,
    mut auth: AuthSession,
    headers: HeaderMap,
    Form(input): Form<DeleteAccount>,
) -> Result<Redirect, AuthError> {
    check_csrf(&state, &auth.session, &headers, &input.csrf).await?;
    let current = user(&auth)?;
    if input.confirm != current.username {
        return Err(AuthError::InvalidInput);
    }
    let id = current.id;
    let mut tx = state.pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(-2)")
        .execute(&mut *tx)
        .await?;
    let role: Option<String> = sqlx::query_scalar("SELECT role FROM users WHERE id=$1 FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
    if role.is_none() {
        return Err(AuthError::Unauthorized);
    }
    if role.as_deref() == Some("admin") {
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM users WHERE role='admin' AND status='approved'",
        )
        .fetch_one(&mut *tx)
        .await?;
        if count <= 1 {
            return Err(AuthError::LastAdmin);
        }
    }
    let legacy: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM legacy_import WHERE user_id=$1)")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
    if legacy {
        sqlx::query("DELETE FROM collection_items")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM collection_stats_history")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM settings")
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query("DELETE FROM app_sessions WHERE data->'axum-login.data'->>'user_id'=$1")
        .bind(id.to_string())
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM users WHERE id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    auth.logout().await.map_err(|_| AuthError::Internal)?;
    Ok(Redirect::to("/auth/login"))
}

pub(super) fn admin(auth: &AuthSession) -> Result<&User, AuthError> {
    let user = user(auth)?;
    if user.role != "admin" || user.status != "approved" {
        return Err(AuthError::Forbidden);
    }
    Ok(user)
}

pub(super) fn approved(auth: &AuthSession) -> Result<&User, AuthError> {
    let user = user(auth)?;
    if user.status != "approved" {
        return Err(AuthError::Forbidden);
    }
    Ok(user)
}

async fn queue_sync(
    State(state): State<AuthState>,
    auth: AuthSession,
    headers: HeaderMap,
    Form(input): Form<Mutation>,
) -> Result<Response, AuthError> {
    let id = approved(&auth)?.id;
    check_csrf(&state, &auth.session, &headers, &input.csrf).await?;
    let mut tx = state.pool.begin().await?;
    let run = waxdemon_db::user_sync::enqueue(&mut tx, id)
        .await?
        .ok_or(AuthError::Forbidden)?;
    tx.commit().await?;
    Ok((
        axum::http::StatusCode::ACCEPTED,
        Json(serde_json::json!({"run_id":run})),
    )
        .into_response())
}

pub(super) async fn sync_status(
    State(state): State<AuthState>,
    auth: AuthSession,
) -> Result<Json<serde_json::Value>, AuthError> {
    let id = approved(&auth)?.id;
    let run: Option<serde_json::Value> = sqlx::query_scalar("SELECT jsonb_build_object('id',id,'status',status,'phase',phase,'processed',processed,'total',total,'attempts',attempts,'error',error,'created_at',created_at,'started_at',started_at,'finished_at',finished_at) FROM user_sync_runs WHERE user_id=$1 ORDER BY id DESC LIMIT 1")
        .bind(id).fetch_optional(&state.pool).await?;
    Ok(Json(serde_json::json!({"run":run})))
}

async fn me(auth: AuthSession) -> Result<Json<serde_json::Value>, AuthError> {
    Ok(Json(
        serde_json::json!({ "user": user(&auth)?, "csrf": csrf(&auth.session).await?, "expires_at": auth.session.get::<i64>(DEADLINE_KEY).await? }),
    ))
}

async fn logout(
    State(state): State<AuthState>,
    mut auth: AuthSession,
    headers: HeaderMap,
    Form(input): Form<Mutation>,
) -> Result<Redirect, AuthError> {
    check_csrf(&state, &auth.session, &headers, &input.csrf).await?;
    user(&auth)?;
    auth.logout().await.map_err(|_| AuthError::Internal)?;
    Ok(Redirect::to("/auth/login"))
}

async fn revoke_sessions(
    State(state): State<AuthState>,
    mut auth: AuthSession,
    headers: HeaderMap,
    Form(input): Form<Mutation>,
) -> Result<Redirect, AuthError> {
    check_csrf(&state, &auth.session, &headers, &input.csrf).await?;
    sqlx::query("UPDATE users SET session_revocation = gen_random_uuid() WHERE id = $1")
        .bind(user(&auth)?.id)
        .execute(&state.pool)
        .await?;
    auth.logout().await.map_err(|_| AuthError::Internal)?;
    Ok(Redirect::to("/auth/login"))
}

async fn decision(
    state: AuthState,
    auth: AuthSession,
    headers: HeaderMap,
    input: Mutation,
    id: i64,
    status: &str,
) -> Result<Redirect, AuthError> {
    check_csrf(&state, &auth.session, &headers, &input.csrf).await?;
    let actor = admin(&auth)?.id;
    let mut tx = state.pool.begin().await?;
    let allowed: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM users WHERE id = $1 AND role = 'admin' AND status = 'approved' FOR UPDATE",
    )
    .bind(actor)
    .fetch_optional(&mut *tx)
    .await?;
    if allowed.is_none() {
        return Err(AuthError::Forbidden);
    }
    let result = sqlx::query("UPDATE users SET status = $2, session_revocation = CASE WHEN $2 = 'rejected' THEN gen_random_uuid() ELSE session_revocation END WHERE id = $1 AND status = 'pending'")
        .bind(id).bind(status).execute(&mut *tx).await?;
    if result.rows_affected() != 1 {
        return Err(AuthError::Conflict);
    }
    if status == "approved" {
        waxdemon_db::user_sync::enqueue(&mut tx, id).await?;
    }
    tx.commit().await?;
    Ok(Redirect::to("/admin/users"))
}

async fn approve(
    State(state): State<AuthState>,
    auth: AuthSession,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Form(input): Form<Mutation>,
) -> Result<Redirect, AuthError> {
    decision(state, auth, headers, input, id, "approved").await
}

async fn reject(
    State(state): State<AuthState>,
    auth: AuthSession,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Form(input): Form<Mutation>,
) -> Result<Redirect, AuthError> {
    decision(state, auth, headers, input, id, "rejected").await
}
