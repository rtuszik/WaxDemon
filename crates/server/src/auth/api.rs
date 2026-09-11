use super::{
    AuthError, AuthSession, AuthState,
    routes::{approved, check_csrf},
};
use axum::{
    Form, Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::get,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Postgres, QueryBuilder};
use waxdemon_core::library::{LibraryItem, LibraryPage, LibraryQuery, Preferences};

const ITEM_COLUMNS: &str = "i.instance_id,i.release_id,r.artist,r.title,r.year,r.format,COALESCE(NULLIF(r.genres,'')::jsonb,'[]'::jsonb) AS genres,COALESCE(NULLIF(r.styles,'')::jsonb,'[]'::jsonb) AS styles,r.cover_image_url,i.added_date,i.folder_id,i.rating,i.notes,i.condition,v.amount::text AS suggested_value,v.currency,COALESCE(i.last_value_check,p.fetched_at::text) AS last_value_check,p.condition AS estimate_condition";
const ITEM_FROM: &str = " FROM user_collection_items i JOIN releases r ON r.id=i.release_id LEFT JOIN LATERAL (SELECT amount,currency,condition,fetched_at FROM user_price_suggestions WHERE user_id=i.user_id AND release_id=i.release_id AND i.suggested_value IS NULL AND NULLIF(trim(i.condition),'') IS NULL ORDER BY fetched_at DESC,currency,array_position(ARRAY['Mint (M)','Near Mint (NM or M-)','Very Good Plus (VG+)','Very Good (VG)','Good Plus (G+)','Good (G)','Fair (F)','Poor (P)'],condition),condition LIMIT 1) p ON true CROSS JOIN LATERAL (SELECT COALESCE(i.suggested_value,p.amount) AS amount,CASE WHEN i.suggested_value IS NOT NULL THEN i.currency ELSE p.currency END AS currency) v";

pub fn router() -> Router<AuthState> {
    Router::new()
        .route("/api/library", get(library))
        .route("/api/library/{instance_id}", get(item))
        .route("/api/library/filters", get(filters))
        .route("/api/dashboard", get(dashboard))
        .route("/api/settings", get(settings).post(save_settings))
}

fn bad_request() -> AuthError {
    AuthError::InvalidInput
}

fn validate(query: &LibraryQuery) -> Result<(u32, u32, &'static str), AuthError> {
    let page = query.page.unwrap_or(1);
    let size = query.page_size.unwrap_or(50);
    if page == 0
        || page > 1_000_000
        || !(1..=100).contains(&size)
        || [
            &query.q,
            &query.genre,
            &query.format,
            &query.condition,
            &query.currency,
        ]
        .into_iter()
        .flatten()
        .any(|v| v.len() > 200)
    {
        return Err(bad_request());
    }
    let sort = match query.sort.as_deref().unwrap_or("added_desc") {
        "added_desc" => "i.added_date DESC,i.instance_id DESC",
        "added_asc" => "i.added_date ASC,i.instance_id ASC",
        "artist" => "r.artist ASC NULLS LAST,r.title ASC NULLS LAST,i.instance_id ASC",
        "title" => "r.title ASC NULLS LAST,r.artist ASC NULLS LAST,i.instance_id ASC",
        "year" => "r.year DESC NULLS LAST,i.instance_id ASC",
        "price_desc" => "v.currency ASC NULLS LAST,v.amount DESC NULLS LAST,i.instance_id ASC",
        "price_asc" => "v.currency ASC NULLS LAST,v.amount ASC NULLS LAST,i.instance_id ASC",
        _ => return Err(bad_request()),
    };
    Ok((page, size, sort))
}

fn literal_search(value: &str) -> String {
    format!(
        "%{}%",
        value
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    )
}

fn predicates<'a>(builder: &mut QueryBuilder<'a, Postgres>, user: i64, query: &'a LibraryQuery) {
    builder
        .push(ITEM_FROM)
        .push(" WHERE i.user_id=")
        .push_bind(user);
    if let Some(q) = query.q.as_deref().filter(|v| !v.is_empty()) {
        builder
            .push(" AND (r.artist ILIKE ")
            .push_bind(literal_search(q))
            .push(" OR r.title ILIKE ")
            .push_bind(literal_search(q))
            .push(")");
    }
    if let Some(year) = query.year {
        builder.push(" AND r.year=").push_bind(year);
    }
    if let Some(folder) = query.folder_id {
        builder.push(" AND i.folder_id=").push_bind(folder);
    }
    if let Some(genre) = query.genre.as_ref().filter(|v| !v.is_empty()) {
        builder
            .push(" AND COALESCE(NULLIF(r.genres,'')::jsonb,'[]'::jsonb) ? ")
            .push_bind(genre);
    }
    if let Some(format) = query.format.as_ref().filter(|v| !v.is_empty()) {
        builder
            .push(" AND r.format ILIKE ")
            .push_bind(literal_search(format));
    }
    if let Some(condition) = query.condition.as_ref().filter(|v| !v.is_empty()) {
        builder.push(" AND i.condition=").push_bind(condition);
    }
    if let Some(currency) = query.currency.as_ref().filter(|v| !v.is_empty()) {
        if currency == "unknown" {
            builder.push(" AND v.currency IS NULL");
        } else {
            builder.push(" AND v.currency=").push_bind(currency);
        }
    }
}

pub(super) async fn library(
    State(state): State<AuthState>,
    auth: AuthSession,
    Query(query): Query<LibraryQuery>,
) -> Result<Json<LibraryPage>, AuthError> {
    let user = approved(&auth)?.id;
    let (page, page_size, sort) = validate(&query)?;
    let mut tx = state.pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let mut count = QueryBuilder::new("SELECT count(*)");
    predicates(&mut count, user, &query);
    let total: i64 = count.build_query_scalar().fetch_one(&mut *tx).await?;
    let mut rows = QueryBuilder::new(format!("SELECT to_jsonb(item) FROM (SELECT {ITEM_COLUMNS}"));
    predicates(&mut rows, user, &query);
    rows.push(" ORDER BY ")
        .push(sort)
        .push(" LIMIT ")
        .push_bind(i64::from(page_size))
        .push(" OFFSET ")
        .push_bind(i64::from(page - 1) * i64::from(page_size))
        .push(") item");
    let items: Vec<Value> = rows.build_query_scalar().fetch_all(&mut *tx).await?;
    let items: Vec<LibraryItem> = items
        .into_iter()
        .map(serde_json::from_value)
        .collect::<Result<_, _>>()
        .map_err(|_| AuthError::Internal)?;
    tx.commit().await?;
    Ok(Json(LibraryPage {
        items,
        total,
        page,
        page_size,
    }))
}

pub(super) async fn item(
    State(state): State<AuthState>,
    auth: AuthSession,
    Path(instance): Path<i64>,
) -> Result<Json<Value>, AuthError> {
    let user = approved(&auth)?.id;
    let query = format!(
        "SELECT to_jsonb(item) FROM (SELECT {ITEM_COLUMNS}{ITEM_FROM} WHERE i.user_id=$1 AND i.instance_id=$2) item"
    );
    let item: Value = sqlx::query_scalar(&query)
        .bind(user)
        .bind(instance)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AuthError::NotFound)?;
    let suggestions:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('condition',condition,'currency',currency,'amount',amount::text,'fetched_at',fetched_at) FROM user_price_suggestions WHERE user_id=$1 AND release_id=$2 ORDER BY currency,array_position(ARRAY['Mint (M)','Near Mint (NM or M-)','Very Good Plus (VG+)','Very Good (VG)','Good Plus (G+)','Good (G)','Fair (F)','Poor (P)'],condition) NULLS LAST,condition")
        .bind(user).bind(item["release_id"].as_i64().ok_or(AuthError::Internal)?).fetch_all(&state.pool).await?;
    Ok(Json(json!({"item":item,"suggestions":suggestions})))
}

pub(super) async fn filters(
    State(state): State<AuthState>,
    auth: AuthSession,
) -> Result<Json<Value>, AuthError> {
    let user = approved(&auth)?.id;
    let filters:Value=sqlx::query_scalar("SELECT jsonb_build_object('years',COALESCE(jsonb_agg(DISTINCT r.year ORDER BY r.year) FILTER(WHERE r.year IS NOT NULL),'[]'),'conditions',COALESCE(jsonb_agg(DISTINCT i.condition ORDER BY i.condition) FILTER(WHERE i.condition IS NOT NULL),'[]'),'currencies',COALESCE(jsonb_agg(DISTINCT i.currency ORDER BY i.currency) FILTER(WHERE i.currency IS NOT NULL),'[]')) FROM user_collection_items i JOIN releases r ON r.id=i.release_id WHERE i.user_id=$1")
        .bind(user).fetch_one(&state.pool).await?;
    let metadata:Option<Value>=sqlx::query_scalar("SELECT jsonb_build_object('folders',folders->'folders','fields',fields->'fields') FROM user_collection_metadata WHERE user_id=$1").bind(user).fetch_optional(&state.pool).await?;
    Ok(Json(json!({"filters":filters,"metadata":metadata})))
}

#[derive(Deserialize)]
pub(super) struct Range {
    range: Option<String>,
}

pub(super) async fn dashboard(
    State(state): State<AuthState>,
    auth: AuthSession,
    Query(range): Query<Range>,
) -> Result<Json<Value>, AuthError> {
    let user = approved(&auth)?.id;
    let days = match range.range.as_deref().unwrap_or("all") {
        "all" => None,
        "1m" => Some(30),
        "3m" => Some(91),
        "6m" => Some(183),
        "1y" => Some(365),
        _ => return Err(bad_request()),
    };
    let total: i64 =
        sqlx::query_scalar("SELECT count(*) FROM user_collection_items WHERE user_id=$1")
            .bind(user)
            .fetch_one(&state.pool)
            .await?;
    let values:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('currency',currency,'valued_items',count(*),'total',sum(suggested_value)::text) FROM user_collection_items WHERE user_id=$1 AND suggested_value IS NOT NULL GROUP BY currency ORDER BY currency")
        .bind(user).fetch_all(&state.pool).await?;
    let history:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('timestamp',timestamp,'total_items',total_items,'minimum',value_min::text,'median',value_median::text,'maximum',value_max::text,'currency',currency) FROM user_collection_history WHERE user_id=$1 AND ($2::int IS NULL OR timestamp::timestamptz>=now()-make_interval(days=>$2)) ORDER BY timestamp::timestamptz")
        .bind(user).bind(days).fetch_all(&state.pool).await?;
    let formats:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('name',COALESCE(r.format,'Unknown'),'count',count(*)) FROM user_collection_items i JOIN releases r ON r.id=i.release_id WHERE i.user_id=$1 GROUP BY r.format ORDER BY count(*) DESC")
        .bind(user).fetch_all(&state.pool).await?;
    let genres:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('name',genre,'count',count(*)) FROM user_collection_items i JOIN releases r ON r.id=i.release_id CROSS JOIN LATERAL jsonb_array_elements_text(COALESCE(NULLIF(r.genres,'')::jsonb,'[]'::jsonb)) genre WHERE i.user_id=$1 GROUP BY genre ORDER BY count(*) DESC")
        .bind(user).fetch_all(&state.pool).await?;
    let display_currency: Option<String> =
        sqlx::query_scalar("SELECT display_currency FROM user_preferences WHERE user_id=$1")
            .bind(user)
            .fetch_optional(&state.pool)
            .await?
            .flatten();
    let summaries: Vec<Value> = sqlx::query_scalar("SELECT to_jsonb(s) FROM (SELECT DISTINCT ON (currency) currency,timestamp,total_items,value_min::text AS minimum,value_median::text AS median,value_max::text AS maximum,(value_median/NULLIF(total_items,0))::text AS average FROM user_collection_history WHERE user_id=$1 ORDER BY currency,timestamp::timestamptz DESC) s")
        .bind(user).fetch_all(&state.pool).await?;
    let years: Vec<Value> = sqlx::query_scalar("SELECT jsonb_build_object('name',CASE WHEN r.year>0 THEN r.year::text ELSE 'Unknown' END,'count',count(*)) FROM user_collection_items i JOIN releases r ON r.id=i.release_id WHERE i.user_id=$1 GROUP BY CASE WHEN r.year>0 THEN r.year::text ELSE 'Unknown' END ORDER BY count(*) DESC,CASE WHEN r.year>0 THEN r.year::text ELSE 'Unknown' END")
        .bind(user).fetch_all(&state.pool).await?;
    let rankings_query = format!(
        "WITH items AS (SELECT {ITEM_COLUMNS},row_number() OVER (PARTITION BY v.currency ORDER BY v.amount DESC,i.instance_id) AS high,row_number() OVER (PARTITION BY v.currency ORDER BY v.amount ASC,i.instance_id) AS low {ITEM_FROM} WHERE i.user_id=$1 AND v.amount IS NOT NULL) SELECT jsonb_build_object('currency',currency,'top',jsonb_agg(to_jsonb(items)-'high'-'low' ORDER BY high) FILTER (WHERE high<=10),'bottom',jsonb_agg(to_jsonb(items)-'high'-'low' ORDER BY low) FILTER (WHERE low<=10)) FROM items WHERE high<=10 OR low<=10 GROUP BY currency ORDER BY currency"
    );
    let rankings: Vec<Value> = sqlx::query_scalar(&rankings_query)
        .bind(user)
        .fetch_all(&state.pool)
        .await?;
    let additions_query = format!(
        "SELECT to_jsonb(item) FROM (SELECT {ITEM_COLUMNS}{ITEM_FROM} WHERE i.user_id=$1 ORDER BY i.added_date DESC,i.instance_id DESC LIMIT 10) item"
    );
    let additions: Vec<Value> = sqlx::query_scalar(&additions_query)
        .bind(user)
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(
        json!({"total_items":total,"values":values,"history":history,"formats":formats,"genres":genres,"years":years,"summaries":summaries,"rankings":rankings,"additions":additions,"display_currency":display_currency}),
    ))
}

pub(super) async fn settings(
    State(state): State<AuthState>,
    auth: AuthSession,
) -> Result<Json<Value>, AuthError> {
    let user = approved(&auth)?.id;
    let prefs:Option<Value>=sqlx::query_scalar("SELECT jsonb_build_object('sync_interval_hours',sync_interval_hours,'price_refresh_hours',price_refresh_hours,'display_currency',display_currency) FROM user_preferences WHERE user_id=$1")
        .bind(user).fetch_optional(&state.pool).await?;
    let connection:Option<Value>=sqlx::query_scalar("SELECT jsonb_build_object('connected_at',connected_at,'updated_at',updated_at) FROM discogs_connections WHERE user_id=$1")
        .bind(user).fetch_optional(&state.pool).await?;
    Ok(Json(
        json!({"preferences":prefs.unwrap_or_else(||json!(Preferences::default())),"connection":connection}),
    ))
}

#[derive(Deserialize)]
struct SaveSettings {
    csrf: String,
    sync_interval_hours: i32,
    price_refresh_hours: i32,
    display_currency: Option<String>,
}

async fn save_settings(
    State(state): State<AuthState>,
    auth: AuthSession,
    headers: HeaderMap,
    Form(input): Form<SaveSettings>,
) -> Result<StatusCode, AuthError> {
    let user = approved(&auth)?.id;
    check_csrf(&state, &auth.session, &headers, &input.csrf).await?;
    let currency = input.display_currency.filter(|v| !v.is_empty());
    if !(0..=720).contains(&input.sync_interval_hours)
        || !(1..=720).contains(&input.price_refresh_hours)
        || currency
            .as_ref()
            .is_some_and(|v| v.len() != 3 || !v.bytes().all(|b| b.is_ascii_uppercase()))
    {
        return Err(bad_request());
    }
    let mut tx = state.pool.begin().await?;
    let allowed: Option<i64> =
        sqlx::query_scalar("SELECT id FROM users WHERE id=$1 AND status='approved' FOR UPDATE")
            .bind(user)
            .fetch_optional(&mut *tx)
            .await?;
    if allowed.is_none() {
        return Err(AuthError::Forbidden);
    }
    sqlx::query("INSERT INTO user_preferences (user_id,sync_interval_hours,price_refresh_hours,display_currency) VALUES ($1,$2,$3,$4) ON CONFLICT (user_id) DO UPDATE SET sync_interval_hours=EXCLUDED.sync_interval_hours,price_refresh_hours=EXCLUDED.price_refresh_hours,display_currency=EXCLUDED.display_currency")
        .bind(user).bind(input.sync_interval_hours).bind(input.price_refresh_hours).bind(currency).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
