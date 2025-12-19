use axum::http::header;
use axum::response::IntoResponse;

// These handlers embed the UI static assets into the binary.
// This avoids "it works on my machine" issues when the process is started
// from a different working directory (ServeDir is relative).

pub async fn ui_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/javascript; charset=utf-8")],
        include_str!("../../static/ui.js"),
    )
}

pub async fn ui_css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../../static/ui.css"),
    )
}
