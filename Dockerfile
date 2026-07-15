# ---- Build stage: compile the Leptos console (WASM) and the Rust control plane ----
FROM rust:1-bookworm AS builder
RUN rustup target add wasm32-unknown-unknown \
    && cargo install trunk --locked
WORKDIR /app
COPY . .
RUN cd crates/web && trunk build --release
RUN cargo build --release -p switchboard-core

# ---- Runtime stage: a slim image with just the binary and the built console ----
FROM debian:bookworm-slim
WORKDIR /app
COPY --from=builder /app/target/release/switchboard /app/switchboard
COPY --from=builder /app/crates/web/dist /app/crates/web/dist
ENV SWITCHBOARD_PORT=7930 \
    SWITCHBOARD_DATA=/app/data
EXPOSE 7930 1883
VOLUME ["/app/data"]
CMD ["/app/switchboard"]
