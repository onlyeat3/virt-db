FROM alpine:3.17.2
RUN sed -i 's/dl-cdn.alpinelinux.org/mirrors.aliyun.com/g' /etc/apk/repositories
RUN apk add vim curl

ADD target/x86_64-unknown-linux-musl/release/admin /app/admin
COPY admin/dist /app/
EXPOSE 8080
ENTRYPOINT ["/app/admin"]