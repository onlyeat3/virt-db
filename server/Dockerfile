FROM alpine:3.17.2
WORKDIR /app
RUN sed -i 's/dl-cdn.alpinelinux.org/mirrors.aliyun.com/g' /etc/apk/repositories
RUN apk add vim curl

COPY target/x86_64-unknown-linux-musl/release/server /app/server

EXPOSE 10101
ENTRYPOINT ["/app/server","-c","/app/conf/config.toml"]