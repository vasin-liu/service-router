FROM rust:1.83-slim AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
COPY benches/ benches/
RUN cargo build --release --bin service-router

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/service-router /usr/local/bin/service-router
COPY config/ /etc/service-router/config/
EXPOSE 8080
ENTRYPOINT ["service-router"]
CMD ["run", "/etc/service-router/config/config.yaml"]
