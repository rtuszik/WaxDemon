use axum::{
    body::Body,
    extract::Path,
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};

struct Asset {
    bytes: &'static [u8],
    content_type: &'static str,
    /// `Cache-Control` value. Vendored libraries at a pinned version are
    /// safe to cache for a year; first-party assets use a short TTL so
    /// redeploys show up quickly.
    cache_control: &'static str,
}

const APEXCHARTS_JS: &[u8] = include_bytes!("../assets/apexcharts.min.js");
const APP_JS: &[u8] = include_bytes!("../assets/app.js");
const STYLES_CSS: &[u8] = include_bytes!("../assets/styles.css");
const FAVICON_ICO: &[u8] = include_bytes!("../assets/favicon.ico");
const FAVICON_16: &[u8] = include_bytes!("../assets/favicon-16x16.png");
const FAVICON_32: &[u8] = include_bytes!("../assets/favicon-32x32.png");
const APPLE_TOUCH_ICON: &[u8] = include_bytes!("../assets/apple-touch-icon.png");
const ANDROID_192: &[u8] = include_bytes!("../assets/android-chrome-192x192.png");
const ANDROID_512: &[u8] = include_bytes!("../assets/android-chrome-512x512.png");
const SITE_WEBMANIFEST: &[u8] = include_bytes!("../assets/site.webmanifest");

const ICON_CACHE: &str = "public, max-age=86400";

fn lookup(name: &str) -> Option<Asset> {
    match name {
        "apexcharts.min.js" => Some(Asset {
            bytes: APEXCHARTS_JS,
            content_type: "application/javascript; charset=utf-8",
            cache_control: "public, max-age=31536000, immutable",
        }),
        "app.js" => Some(Asset {
            bytes: APP_JS,
            content_type: "application/javascript; charset=utf-8",
            cache_control: "public, max-age=3600",
        }),
        "styles.css" => Some(Asset {
            bytes: STYLES_CSS,
            content_type: "text/css; charset=utf-8",
            cache_control: "public, max-age=3600",
        }),
        "favicon.ico" => Some(Asset {
            bytes: FAVICON_ICO,
            content_type: "image/x-icon",
            cache_control: ICON_CACHE,
        }),
        "favicon-16x16.png" => Some(Asset {
            bytes: FAVICON_16,
            content_type: "image/png",
            cache_control: ICON_CACHE,
        }),
        "favicon-32x32.png" => Some(Asset {
            bytes: FAVICON_32,
            content_type: "image/png",
            cache_control: ICON_CACHE,
        }),
        "apple-touch-icon.png" => Some(Asset {
            bytes: APPLE_TOUCH_ICON,
            content_type: "image/png",
            cache_control: ICON_CACHE,
        }),
        "android-chrome-192x192.png" => Some(Asset {
            bytes: ANDROID_192,
            content_type: "image/png",
            cache_control: ICON_CACHE,
        }),
        "android-chrome-512x512.png" => Some(Asset {
            bytes: ANDROID_512,
            content_type: "image/png",
            cache_control: ICON_CACHE,
        }),
        "site.webmanifest" => Some(Asset {
            bytes: SITE_WEBMANIFEST,
            content_type: "application/manifest+json",
            cache_control: ICON_CACHE,
        }),
        _ => None,
    }
}

pub async fn handler(Path(name): Path<String>) -> Response {
    match lookup(&name) {
        Some(asset) => (
            StatusCode::OK,
            [
                (
                    header::CONTENT_TYPE,
                    HeaderValue::from_static(asset.content_type),
                ),
                (
                    header::CACHE_CONTROL,
                    HeaderValue::from_static(asset.cache_control),
                ),
            ],
            Body::from(asset.bytes),
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

pub async fn favicon() -> Response {
    handler(Path("favicon.ico".to_string())).await
}
