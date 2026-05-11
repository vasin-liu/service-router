use axum::{body::Body, extract::Request, response::Response};
use reqwest::Client;
use tracing::debug;

use crate::error::ProxyError;

/// Forward an HTTP request to `{upstream_base}{rewritten_path}`, preserving
/// method, headers, and body. Returns the upstream response as an Axum
/// `Response`.
pub async fn proxy_http(
    req: Request,
    client: &Client,
    upstream_base: &str,
    rewritten_path: &str,
    extra_response_headers: Option<&[(http::HeaderName, http::HeaderValue)]>,
) -> Result<Response, ProxyError> {
    // Build the upstream URL, preserving the original query string.
    let upstream_url = build_upstream_url(upstream_base, rewritten_path, req.uri().query());

    debug!(upstream_url = %upstream_url, "Forwarding HTTP request");

    let method = req.method().clone();
    let headers = req.headers().clone();

    // Collect the request body.
    let body_bytes = axum::body::to_bytes(req.into_body(), usize::MAX)
        .await
        .map_err(|e| ProxyError::BodyRead(e.to_string()))?;

    // Build the upstream request.
    let mut upstream_req = client.request(method, &upstream_url);

    // Forward headers, filtering out hop-by-hop headers.
    for (name, value) in &headers {
        if !is_hop_by_hop(name.as_str()) {
            upstream_req = upstream_req.header(name, value);
        }
    }

    upstream_req = upstream_req.body(body_bytes);

    // Send and get response.
    let upstream_resp = upstream_req
        .send()
        .await
        .map_err(|e| ProxyError::UpstreamConnection(e.to_string()))?;

    // Convert the upstream response into an Axum response.
    let status = upstream_resp.status();
    let resp_headers = upstream_resp.headers().clone();

    let body_bytes = upstream_resp
        .bytes()
        .await
        .map_err(|e| ProxyError::UpstreamConnection(e.to_string()))?;

    let mut builder = Response::builder().status(status);
    let headers_map = builder.headers_mut().unwrap();

    for (name, value) in &resp_headers {
        if !is_hop_by_hop(name.as_str()) {
            headers_map.insert(name, value.clone());
        }
    }

    if let Some(extra) = extra_response_headers {
        for (name, value) in extra {
            headers_map.insert(name, value.clone());
        }
    }

    builder
        .body(Body::from(body_bytes))
        .map_err(|e| ProxyError::UpstreamConnection(e.to_string()))
}

fn build_upstream_url(base: &str, path: &str, query: Option<&str>) -> String {
    let base = base.trim_end_matches('/');
    let path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{}", path)
    };
    match query {
        Some(q) if !q.is_empty() => format!("{}{}?{}", base, path, q),
        _ => format!("{}{}", base, path),
    }
}

/// Headers users may not set via `routing_rule.response_headers` (framing /
/// hop-by-hop); validated while compiling routing rules.
pub(crate) fn is_forbidden_config_response_header(name: &str) -> bool {
    is_hop_by_hop(name) || name.eq_ignore_ascii_case("content-length")
}

/// Returns `true` for HTTP/1.1 hop-by-hop headers that must not be forwarded.
fn is_hop_by_hop(name: &str) -> bool {
    matches!(
        name.to_lowercase().as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailers"
            | "transfer-encoding"
            | "upgrade"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn extra_response_headers_merge_and_override_upstream() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 8192];
            let mut total = 0usize;
            loop {
                let n = stream.read(&mut buf[total..]).await.expect("read");
                assert!(n > 0, "client closed before headers");
                total += n;
                if buf[..total].windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
                assert!(total < buf.len(), "request too large");
            }
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nX-Merge: upstream\r\nX-Override: old\r\nConnection: close\r\n\r\n",
                )
                .await
                .unwrap();
        });

        let upstream_base = format!("http://{}", addr);
        let uri: http::Uri = format!("http://127.0.0.1:{}/incoming", addr.port())
            .parse()
            .unwrap();
        let req = Request::builder()
            .method("GET")
            .uri(uri)
            .body(Body::empty())
            .unwrap();

        let extra = [
            (
                http::HeaderName::from_static("x-added"),
                http::HeaderValue::from_static("by-proxy"),
            ),
            (
                http::HeaderName::from_static("x-override"),
                http::HeaderValue::from_static("new"),
            ),
        ];

        let client = Client::new();
        let resp = proxy_http(req, &client, &upstream_base, "/", Some(&extra))
            .await
            .unwrap();

        assert_eq!(resp.headers().get("x-merge").unwrap(), "upstream");
        assert_eq!(resp.headers().get("x-added").unwrap(), "by-proxy");
        assert_eq!(resp.headers().get("x-override").unwrap(), "new");
    }

    #[test]
    fn test_build_upstream_url_no_query() {
        assert_eq!(
            build_upstream_url("http://host:8080", "/api/v1/users", None),
            "http://host:8080/api/v1/users"
        );
    }

    #[test]
    fn test_build_upstream_url_with_query() {
        assert_eq!(
            build_upstream_url("http://host:8080", "/search", Some("q=hello")),
            "http://host:8080/search?q=hello"
        );
    }

    #[test]
    fn test_build_upstream_url_trailing_slash_base() {
        assert_eq!(
            build_upstream_url("http://host:8080/", "/path", None),
            "http://host:8080/path"
        );
    }

    #[test]
    fn test_hop_by_hop() {
        assert!(is_hop_by_hop("connection"));
        assert!(is_hop_by_hop("Transfer-Encoding"));
        assert!(!is_hop_by_hop("content-type"));
        assert!(!is_hop_by_hop("x-request-id"));
    }
}
