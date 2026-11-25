use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, RwLock,
    },
};

use axum::{
    extract::{rejection::JsonRejection, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct CreateLinkRequest {
    pub long_url: String,
}

#[derive(Debug, Serialize)]
pub struct LinkResponse {
    pub short_url: String,
    pub created_at: DateTime<Utc>,
    pub hits: u64,
}

#[derive(Debug)]
pub enum AppError {
    BadRequest(String),
    NotFound,
    Internal,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            Self::NotFound => (StatusCode::NOT_FOUND, "link not found".to_owned()),
            Self::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal server error".to_owned(),
            ),
        };

        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

#[derive(Clone)]
pub struct AppState {
    links: Arc<RwLock<HashMap<String, StoredLink>>>,
    next_id: Arc<AtomicU64>,
    base_url: Arc<str>,
}

struct StoredLink {
    long_url: String,
    hits: u64,
}

impl AppState {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            links: Arc::new(RwLock::new(HashMap::new())),
            next_id: Arc::new(AtomicU64::new(1)),
            base_url: Arc::from(base_url.into().trim_end_matches('/').to_owned()),
        }
    }
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/api/shorten", post(create_link))
        .route("/api/links/{id}", get(redirect_link))
        .with_state(state)
}

async fn create_link(
    State(state): State<AppState>,
    payload: Result<Json<CreateLinkRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<LinkResponse>), AppError> {
    let Json(request) = payload.map_err(|error| AppError::BadRequest(error.body_text()))?;
    let parsed_url = url::Url::parse(&request.long_url)
        .map_err(|_| AppError::BadRequest("long_url must be a valid HTTP or HTTPS URL".into()))?;

    if !matches!(parsed_url.scheme(), "http" | "https") || parsed_url.host().is_none() {
        return Err(AppError::BadRequest(
            "long_url must be a valid HTTP or HTTPS URL".into(),
        ));
    }

    let id = to_base36(state.next_id.fetch_add(1, Ordering::Relaxed));
    let created_at = Utc::now();
    let short_url = format!("{}/api/links/{id}", state.base_url);
    let link = StoredLink {
        long_url: parsed_url.to_string(),
        hits: 0,
    };

    state
        .links
        .write()
        .map_err(|_| AppError::Internal)?
        .insert(id, link);

    Ok((
        StatusCode::CREATED,
        Json(LinkResponse {
            short_url,
            created_at,
            hits: 0,
        }),
    ))
}

async fn redirect_link(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, AppError> {
    let long_url = {
        let mut links = state.links.write().map_err(|_| AppError::Internal)?;
        let link = links.get_mut(&id).ok_or(AppError::NotFound)?;
        link.hits += 1;
        link.long_url.clone()
    };

    let location = long_url
        .parse::<axum::http::HeaderValue>()
        .map_err(|_| AppError::Internal)?;
    Ok((StatusCode::FOUND, [("location", location)]).into_response())
}

fn to_base36(mut value: u64) -> String {
    let mut result = Vec::new();
    while value > 0 {
        let digit = (value % 36) as u8;
        result.push(if digit < 10 {
            b'0' + digit
        } else {
            b'a' + digit - 10
        });
        value /= 36;
    }
    result.reverse();
    String::from_utf8(result).expect("base36 digits are valid UTF-8")
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use serde_json::Value;
    use tower::ServiceExt;

    use super::{app, AppState};

    #[tokio::test]
    async fn create_link_returns_documented_response_and_redirects() {
        let app = app(AppState::new("http://localhost:3000"));
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/shorten")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"long_url":"https://example.com/path"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let created: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(created["short_url"], "http://localhost:3000/api/links/1");
        assert_eq!(created["hits"], 0);
        assert!(created["created_at"].as_str().is_some());

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/links/1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FOUND);
        assert_eq!(
            response.headers().get("location").unwrap(),
            "https://example.com/path"
        );
    }

    #[tokio::test]
    async fn missing_link_returns_404() {
        let response = app(AppState::new("http://localhost:3000"))
            .oneshot(
                Request::builder()
                    .uri("/api/links/missing")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn invalid_url_returns_400() {
        let response = app(AppState::new("http://localhost:3000"))
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/shorten")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"long_url":"ftp://example.com"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
