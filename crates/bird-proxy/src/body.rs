use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::header::{CONTENT_TYPE, HeaderValue};
use hyper::{Response, StatusCode};

pub(crate) type ProxyBody = BoxBody<Bytes, hyper::Error>;

pub(crate) fn error_response(status: StatusCode, message: &'static str) -> Response<ProxyBody> {
    let body = Full::new(Bytes::from_static(message.as_bytes()))
        .map_err(|never| match never {})
        .boxed();
    let mut response = Response::new(body);
    *response.status_mut() = status;
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    response
}
