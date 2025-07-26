FROM rust:alpine AS builder
WORKDIR /app
COPY . .

RUN cargo install --path .

FROM alpine:latest
LABEL authors="niloy"
COPY --from=builder /usr/local/cargo/bin/rust_test /usr/local/bin/rust_test
CMD ["rust_test"]
