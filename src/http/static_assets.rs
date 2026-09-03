use axum::http::header;
use axum::response::IntoResponse;

// These handlers embed the UI static assets into the binary.
// This avoids "it works on my machine" issues when the process is started
// from a different working directory (ServeDir is relative).

pub async fn ui_js() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        include_str!("../../static/ui.js"),
    )
}

pub async fn ui_css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../../static/ui.css"),
    )
}

pub async fn tabler_css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../../static/vendor/tabler/tabler.min.css"),
    )
}

pub async fn tabler_js() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        include_str!("../../static/vendor/tabler/tabler.min.js"),
    )
}

pub async fn tabler_icons_css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../../static/vendor/tabler-icons/tabler-icons.min.css"),
    )
}

pub async fn tabler_icons_eot() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/vnd.ms-fontobject")],
        include_bytes!("../../static/vendor/tabler-icons/fonts/tabler-icons.eot").as_slice(),
    )
}

pub async fn tabler_icons_ttf() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "font/ttf")],
        include_bytes!("../../static/vendor/tabler-icons/fonts/tabler-icons.ttf").as_slice(),
    )
}

pub async fn tabler_icons_woff() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "font/woff")],
        include_bytes!("../../static/vendor/tabler-icons/fonts/tabler-icons.woff").as_slice(),
    )
}

pub async fn tabler_icons_woff2() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "font/woff2")],
        include_bytes!("../../static/vendor/tabler-icons/fonts/tabler-icons.woff2").as_slice(),
    )
}
