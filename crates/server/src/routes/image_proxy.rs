use crate::{error::AppError, state::AppState};
use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Params {
    pub url: Option<String>,
}

pub async fn handler(
    State(st): State<AppState>,
    Query(params): Query<Params>,
) -> Result<Response, AppError> {
    let url = params
        .url
        .ok_or_else(|| AppError::BadRequest("Missing image URL parameter".into()))?;

    // Whitelist: only `https://i.discogs.com/`
    if url.starts_with('/') {
        return Err(AppError::BadRequest(
            "Proxying local files is not supported".into(),
        ));
    }
    if !url.starts_with(&st.image_proxy_prefix) {
        return Err(AppError::BadRequest("Invalid image URL provided".into()));
    }

    let upstream = st
        .http
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::Upstream {
            status: 502,
            msg: e.to_string(),
        })?;

    if !upstream.status().is_success() {
        let code = upstream.status().as_u16();
        let status = if (400..600).contains(&code) {
            code
        } else {
            502
        };
        return Err(AppError::Upstream {
            status,
            msg: upstream
                .status()
                .canonical_reason()
                .unwrap_or("upstream error")
                .to_string(),
        });
    }

    let content_type = upstream
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();

    let content_length = upstream
        .headers()
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&content_type)
            .unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    if let Some(cl) = content_length {
        if let Ok(v) = HeaderValue::from_str(&cl) {
            headers.insert(header::CONTENT_LENGTH, v);
        }
    }
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=604800, immutable"),
    );

    let stream = upstream.bytes_stream();
    let body = Body::from_stream(stream);

    Ok((StatusCode::OK, headers, body).into_response())
}
