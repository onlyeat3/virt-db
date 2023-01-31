FROM clux/muslrust:stable AS builder

COPY . .

RUN echo '[source.crates-io]\n\
           replace-with = "tuna"\n\
           \n\
           [source.tuna]\n\
           registry = "https://mirrors.tuna.tsinghua.edu.cn/git/crates.io-index.git"' \
    >> .cargo/config.toml
RUN rustup default nightly \
    && rustup target add x86_64-unknown-linux-musl \
    && cargo +nightly build --target=x86_64-unknown-linux-musl --release --bin server


FROM alpine:3.17.1
RUN apk add vim curl

COPY --from=builder /volume/target/x86_64-unknown-linux-musl/release/server /app/
EXPOSE 10101
ENTRYPOINT ["/app/server"]