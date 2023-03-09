FROM alpine:3.17.2
RUN sed -i 's/dl-cdn.alpinelinux.org/mirrors.aliyun.com/g' /etc/apk/repositories
RUN apk add vim curl

ADD target/x86_64-unknown-linux-musl/release/server /app
RUN ls /app
EXPOSE 10101
ENTRYPOINT ["/app/server"]