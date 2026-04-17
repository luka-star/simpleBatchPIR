# Change from 1.74 to 1.84
FROM rust:1.84-slim as builder
WORKDIR /usr/src/app
COPY . .
RUN cargo build --release -p server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y libpq-dev ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/src/app/target/release/server /usr/local/bin/server
CMD ["server"]