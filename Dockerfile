# Build Stage
FROM rust:1-slim-bookworm AS builder
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
COPY . .
RUN cargo build --release

# Runtime Stage
FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/transaction-ledger-api /app/transaction-ledger-api
COPY --from=builder /app/static /app/static
EXPOSE 3000
ENV PORT=3000
CMD ["/app/transaction-ledger-api"]
