//! Platform logos bundled at compile time (no network at runtime).

use crate::serve::theme::UiTheme;
use crate::serve::themed_logos;
use axum::{
    extract::Path,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};

/// URL path for the dashboard `<img src>`.
pub fn local_logo_path(platform_id: &str, theme: UiTheme) -> String {
    match theme {
        UiTheme::Default => format!("/logos/{platform_id}"),
        t => format!("/logos/{}/{}", t.id(), platform_id),
    }
}

pub fn logo_bytes(platform_id: &str) -> Option<(&'static str, &'static [u8])> {
    match platform_id {
        "claude_code" => Some(("image/png", include_bytes!("assets/logos/claude_code.png"))),
        "codex" => Some(("image/png", include_bytes!("assets/logos/codex.png"))),
        "cursor" => Some(("image/png", include_bytes!("assets/logos/cursor.png"))),
        "opencode" => Some(("image/x-icon", include_bytes!("assets/logos/opencode.ico"))),
        "openclaw" => Some(("image/png", include_bytes!("assets/logos/openclaw.png"))),
        "hermes" => Some(("image/x-icon", include_bytes!("assets/logos/hermes.ico"))),
        "qwen_code" => Some(("image/png", include_bytes!("assets/logos/qwen_code.png"))),
        "cline" => Some(("image/png", include_bytes!("assets/logos/cline.png"))),
        "kilo_cli" => Some(("image/png", include_bytes!("assets/logos/kilo_cli.png"))),
        "kilo_ide" => Some(("image/png", include_bytes!("assets/logos/kilo_cli.png"))),
        "cherry_studio" => Some(("image/png", include_bytes!("assets/logos/cherry_studio.png"))),
        "chatbox" => Some(("image/x-icon", include_bytes!("assets/logos/chatbox.ico"))),
        "qoder" | "qoder_cn" => Some(("image/png", include_bytes!("assets/logos/qoder.png"))),
        "postman" => Some(("image/x-icon", include_bytes!("assets/logos/postman.ico"))),
        "dify" => Some(("image/png", include_bytes!("assets/logos/dify.png"))),
        _ => None,
    }
}

fn themed_or_default(theme: &str, platform_id: &str) -> Option<(&'static str, &'static [u8])> {
    if theme != "default" {
        if let Some(hit) = themed_logos::themed_logo_bytes(theme, platform_id) {
            return Some(hit);
        }
    }
    logo_bytes(platform_id)
}

pub async fn serve_logo(Path(id): Path<String>) -> Response {
    let base = id.split('.').next().unwrap_or(id.as_str());
    match logo_bytes(base) {
        Some((mime, data)) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, mime)],
            data,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

pub async fn serve_themed_logo(Path((theme, id)): Path<(String, String)>) -> Response {
    let base = id.split('.').next().unwrap_or(id.as_str());
    match themed_or_default(&theme, base) {
        Some((mime, data)) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, mime)],
            data,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
