FROM clux/muslrust:stable AS builder

COPY . .
RUN cargo build --release --bin server

FROM alpine:3.17.1
RUN apk add vim curl

COPY --from=builder /volume/target/x86_64-unknown-linux-musl/release/server /app/
EXPOSE 10101
ENTRYPOINT ["/app/server"]