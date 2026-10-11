use axum::http::header::{
    CACHE_CONTROL, CONTENT_SECURITY_POLICY, CONTENT_TYPE, X_CONTENT_TYPE_OPTIONS,
};
use axum::http::{Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};

include!(concat!(env!("OUT_DIR"), "/dashboard.rs"));

const INDEX: &str = "/index.html";
const NOT_BUILT: &str = "this birdd was built without its dashboard: run `pnpm build` in frontend/, then build birdd again\n";

// serves the embedded dashboard; paths that are not files are its own routes, so they get index.html
pub(crate) async fn serve(method: Method, uri: Uri) -> Response {
    if method != Method::GET && method != Method::HEAD {
        return StatusCode::NOT_FOUND.into_response();
    }
    match resolve(FILES, uri.path()) {
        Resolved::File(path, bytes) => file(path, bytes),
        Resolved::NotFound => StatusCode::NOT_FOUND.into_response(),
        Resolved::NotBuilt => (
            StatusCode::NOT_FOUND,
            [(CONTENT_TYPE, "text/plain; charset=utf-8")],
            NOT_BUILT,
        )
            .into_response(),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Resolved {
    File(&'static str, &'static [u8]),
    NotFound,
    NotBuilt,
}

fn resolve(files: &'static [(&'static str, &'static [u8])], path: &str) -> Resolved {
    if let Some(&(path, bytes)) = files.iter().find(|(name, _)| *name == path) {
        return Resolved::File(path, bytes);
    }
    // a missing api route or asset is a real 404, not a page of the app
    let is_page = path != "/v1"
        && !path.starts_with("/v1/")
        && !path.starts_with("/assets/")
        && !has_extension(path);
    if !is_page {
        return Resolved::NotFound;
    }
    match files.iter().find(|(name, _)| *name == INDEX) {
        Some(&(path, bytes)) => Resolved::File(path, bytes),
        None => Resolved::NotBuilt,
    }
}

fn has_extension(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .is_some_and(|name| name.contains('.'))
}

fn file(path: &str, bytes: &'static [u8]) -> Response {
    // vite puts a content hash in every asset name, so they never change
    let cache = if path.starts_with("/assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    (
        [
            (CONTENT_TYPE, content_type(path)),
            (CACHE_CONTROL, cache),
            (X_CONTENT_TYPE_OPTIONS, "nosniff"),
            (CONTENT_SECURITY_POLICY, "frame-ancestors 'none'"),
        ],
        bytes,
    )
        .into_response()
}

fn content_type(path: &str) -> &'static str {
    match path.rsplit_once('.').map(|(_, extension)| extension) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json" | "map") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("txt") => "text/plain; charset=utf-8",
        Some("webmanifest") => "application/manifest+json",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static BUILT: &[(&str, &[u8])] = &[
        ("/assets/index-abc.js", b"js"),
        ("/favicon-16.png", b"png"),
        ("/index.html", b"html"),
    ];

    #[test]
    fn serves_files_by_path() {
        assert_eq!(
            resolve(BUILT, "/assets/index-abc.js"),
            Resolved::File("/assets/index-abc.js", b"js")
        );
        assert_eq!(
            resolve(BUILT, "/favicon-16.png"),
            Resolved::File("/favicon-16.png", b"png")
        );
    }

    #[test]
    fn answers_app_routes_with_the_index() {
        for path in [
            "/",
            "/projects/prod/production",
            "/account",
            "/admin/users/ada",
        ] {
            assert_eq!(
                resolve(BUILT, path),
                Resolved::File(INDEX, b"html"),
                "{path}"
            );
        }
    }

    #[test]
    fn missing_api_routes_and_files_stay_404() {
        for path in [
            "/v1",
            "/v1/nope",
            "/assets/gone-123.js",
            "/robots.txt",
            "/a/b.png",
        ] {
            assert_eq!(resolve(BUILT, path), Resolved::NotFound, "{path}");
        }
    }

    #[test]
    fn says_when_the_dashboard_was_not_built() {
        assert_eq!(resolve(&[], "/"), Resolved::NotBuilt);
        assert_eq!(resolve(&[], "/v1/nope"), Resolved::NotFound);
    }

    #[test]
    fn caches_hashed_assets_forever_and_revalidates_the_rest() {
        let asset = file("/assets/index-abc.js", b"js");
        assert_eq!(
            asset.headers()[CACHE_CONTROL],
            "public, max-age=31536000, immutable"
        );
        assert_eq!(
            asset.headers()[CONTENT_TYPE],
            "text/javascript; charset=utf-8"
        );
        let index = file(INDEX, b"html");
        assert_eq!(index.headers()[CACHE_CONTROL], "no-cache");
        assert_eq!(index.headers()[X_CONTENT_TYPE_OPTIONS], "nosniff");
        assert_eq!(index.headers()[CONTENT_TYPE], "text/html; charset=utf-8");
    }
}
