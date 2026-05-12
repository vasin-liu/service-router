use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::Request;
use reqwest::Client;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::runtime::Runtime;

use service_router::proxy::http_proxy;

const RESPONSE_200: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: keep-alive\r\n\r\nok";

async fn spawn_mock_upstream() -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                let mut buf = vec![0u8; 16384];
                loop {
                    let mut total = 0;
                    loop {
                        let n = match stream.read(&mut buf[total..]).await {
                            Ok(0) => return,
                            Err(_) => return,
                            Ok(n) => n,
                        };
                        total += n;
                        if buf[..total].windows(4).any(|w| w == b"\r\n\r\n") {
                            break;
                        }
                        if total >= buf.len() {
                            return;
                        }
                    }

                    // Drain body if Content-Length present
                    let header_str = String::from_utf8_lossy(&buf[..total]);
                    let content_len: usize = header_str
                        .lines()
                        .find_map(|l| {
                            let lower = l.to_lowercase();
                            if lower.starts_with("content-length:") {
                                l.split(':').nth(1)?.trim().parse().ok()
                            } else {
                                None
                            }
                        })
                        .unwrap_or(0);

                    let header_end = buf[..total]
                        .windows(4)
                        .position(|w| w == b"\r\n\r\n")
                        .unwrap()
                        + 4;
                    let body_read = total - header_end;
                    let remaining = content_len.saturating_sub(body_read);
                    if remaining > 0 {
                        let mut discard = vec![0u8; remaining];
                        if stream.read_exact(&mut discard).await.is_err() {
                            return;
                        }
                    }

                    if stream.write_all(RESPONSE_200).await.is_err() {
                        return;
                    }
                }
            });
        }
    });
    addr
}

fn bench_proxy_http(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let (addr, client) = rt.block_on(async {
        let addr = spawn_mock_upstream().await;
        let client = Client::builder()
            .pool_max_idle_per_host(64)
            .build()
            .unwrap();
        (addr, client)
    });

    let upstream_base = format!("http://127.0.0.1:{}", addr.port());

    let mut group = c.benchmark_group("proxy_http");

    for body_size in [0usize, 256, 4096] {
        group.bench_with_input(
            BenchmarkId::new("body_bytes", body_size),
            &body_size,
            |b, &size| {
                b.to_async(&rt).iter(|| {
                    let client = client.clone();
                    let base = upstream_base.clone();
                    let body = vec![b'x'; size];
                    async move {
                        let req = Request::builder()
                            .method("POST")
                            .uri("/api/test")
                            .header("content-type", "application/octet-stream")
                            .body(Body::from(body))
                            .unwrap();
                        http_proxy::proxy_http(req, &client, &base, "/api/test", None, "bench-id")
                            .await
                            .unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_proxy_http);
criterion_main!(benches);
