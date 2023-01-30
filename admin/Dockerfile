FROM clux/muslrust:stable AS builder

COPY . .
RUN cargo build --release --bin admin

FROM alpine:3.17.1
RUN apk add vim curl

COPY --from=builder /volume/target/x86_64-unknown-linux-musl/release/admin /app/
EXPOSE 8080
ENTRYPOINT ["/app/admin"]