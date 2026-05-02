use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),

    #[error("upstream {status}: {msg}")]
    Upstream { status: u16, msg: String },

    #[error("internal: {0}")]
    Internal(String),
}

#[derive(Serialize)]
struct ErrorBody {
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message, detail) = match &self {
            AppError::BadRequest(m) => (StatusCode::BAD_REQUEST, m.clone(), None),
            AppError::Upstream { status, msg } => (
                StatusCode::from_u16(*status).unwrap_or(StatusCode::BAD_GATEWAY),
                format!("Failed to fetch image: {msg}"),
                None,
            ),
            AppError::Internal(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error".into(),
                Some(e.clone()),
            ),
        };
        (
            status,
            Json(ErrorBody {
                message,
                error: detail,
            }),
        )
            .into_response()
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::Internal(e.to_string())
    }
}

impl From<waxdemon_db::DbError> for AppError {
    fn from(e: waxdemon_db::DbError) -> Self {
        AppError::Internal(e.to_string())
    }
}
