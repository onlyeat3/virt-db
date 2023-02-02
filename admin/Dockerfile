
FROM clux/muslrust:stable AS builder-rust
ARG branch

COPY . .
RUN echo '[source.crates-io]\n\
           replace-with = "tuna"\n\
           \n\
           [source.tuna]\n\
           registry = "https://mirrors.tuna.tsinghua.edu.cn/git/crates.io-index.git"' \
    >> .cargo/config.toml
RUN rustup default nightly \
    && rustup target add x86_64-unknown-linux-musl \
    && cargo +nightly build --target=x86_64-unknown-linux-musl --release --bin admin


FROM alpine:3.17.1
RUN apk add vim curl

COPY --from=builder-rust /volume/target/x86_64-unknown-linux-musl/release/admin /app/
COPY admin/dist /app/
EXPOSE 8080
ENTRYPOINT ["/app/admin"]