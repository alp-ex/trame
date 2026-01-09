use std::sync::Arc;

use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::{Method, Request, Response, StatusCode};

use crate::handlers;
use crate::AppState;

pub struct Router;

impl Router {
    pub async fn handle(
        req: Request<Incoming>,
        state: Arc<AppState>,
    ) -> Result<Response<Full<Bytes>>, hyper::Error> {
        let method = req.method().clone();
        let path = req.uri().path().to_string();
        let origin = &state.config.allowed_origin;
        let auth_header = req
            .headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        // Read body
        let body = req.collect().await?.to_bytes();
        let body_str = String::from_utf8_lossy(&body).to_string();

        let result = match (method, path.as_str()) {
            // Public routes
            (Method::POST, "/api/signup") => handlers::signup(&state, &body_str),
            (Method::POST, "/api/login") => handlers::login(&state, &body_str),

            // Protected routes
            (Method::POST, "/api/logout") => {
                let token = auth_header
                    .as_ref()
                    .and_then(|h| h.strip_prefix("Bearer "))
                    .unwrap_or("");
                handlers::logout(&state, token)
            }
            (Method::GET, "/api/note") => {
                match handlers::authenticate(&state, auth_header.as_deref()) {
                    Ok(auth) => handlers::get_note(&state, &auth.user_id),
                    Err(e) => Err(e),
                }
            }
            (Method::PUT, "/api/note") => {
                match handlers::authenticate(&state, auth_header.as_deref()) {
                    Ok(auth) => handlers::update_note(&state, &auth.user_id, &body_str),
                    Err(e) => Err(e),
                }
            }

            // Health check
            (Method::GET, "/api/health") => Ok(r#"{"status":"ok"}"#.to_string()),

            // CORS preflight
            (Method::OPTIONS, _) => return Ok(cors_preflight(origin)),

            // Serve frontend
            (Method::GET, "/") | (Method::GET, "/index.html") => {
                return Ok(serve_html());
            }

            // Not found
            _ => Err((404, r#"{"error":"Not found"}"#.to_string())),
        };

        let (status, body) = match result {
            Ok(body) => (StatusCode::OK, body),
            Err((code, body)) => (
                StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                body,
            ),
        };

        Ok(json_response(status, &body, origin))
    }
}

fn json_response(status: StatusCode, body: &str, origin: &str) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .header("Access-Control-Allow-Origin", origin)
        .header("Access-Control-Allow-Methods", "GET, POST, PUT, OPTIONS")
        .header(
            "Access-Control-Allow-Headers",
            "Content-Type, Authorization",
        )
        .body(Full::new(Bytes::from(body.to_string())))
        .unwrap()
}

fn cors_preflight(origin: &str) -> Response<Full<Bytes>> {
    Response::builder()
        .status(StatusCode::OK)
        .header("Access-Control-Allow-Origin", origin)
        .header("Access-Control-Allow-Methods", "GET, POST, PUT, OPTIONS")
        .header(
            "Access-Control-Allow-Headers",
            "Content-Type, Authorization",
        )
        .body(Full::new(Bytes::new()))
        .unwrap()
}

fn serve_html() -> Response<Full<Bytes>> {
    const HTML: &str = include_str!("../../web/index.html");
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/html; charset=utf-8")
        .body(Full::new(Bytes::from(HTML)))
        .unwrap()
}
