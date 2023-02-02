FROM clux/muslrust:stable AS builder

ARG branch=main
COPY . .
RUN git clone https://github.com/onlyeat3/virt-db-admin-ui \
    && cd virt-db-admin-ui \
    && git checkout  ${main}\
    && pnpm install \
    && pnpm build
RUN cargo build --release --bin admin

FROM alpine:3.17.1
RUN apk add vim curl

COPY --from=builder /volume/target/x86_64-unknown-linux-musl/release/admin /app/
COPY --from=builder /volume/virt-db-admin-ui/dist /app/
RUN ls -al /app
EXPOSE 8080
ENTRYPOINT ["/app/admin"]