use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::response::IntoResponse;

// pinned with subresource integrity so the cdn can only ever serve this exact build
const PAGE: &str = r#"<!doctype html>
<html>
<head>
    <title>bird API</title>
    <meta charset="utf-8"/>
    <meta name="viewport" content="width=device-width, initial-scale=1"/>
</head>
<body>
<script id="api-reference" data-url="/v1/openapi.json"></script>
<script
    src="https://cdn.jsdelivr.net/npm/@scalar/api-reference@1.72.4/dist/browser/standalone.js"
    integrity="sha384-omTRdD9MbjA1vm12DqRUVvqJlr3VzSixvAdF1Jruu9AJOiJKyTKraIB6DyX+m10M"
    crossorigin="anonymous"></script>
</body>
</html>
"#;

pub(crate) async fn page() -> impl IntoResponse {
    (
        [
            (CONTENT_TYPE, "text/html; charset=utf-8"),
            (CACHE_CONTROL, "no-cache"),
        ],
        PAGE,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_our_spec_with_a_pinned_verified_script() {
        assert!(PAGE.contains(r#"data-url="/v1/openapi.json""#));
        assert!(PAGE.contains("@scalar/api-reference@1.72.4/"));
        assert!(PAGE.contains(r#"integrity="sha384-"#));
        assert!(PAGE.contains(r#"crossorigin="anonymous""#));
    }
}
