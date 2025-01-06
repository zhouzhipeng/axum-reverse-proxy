FROM --platform=linux/amd64 rust:latest as builder
WORKDIR /usr/src/app
COPY . .
RUN cargo build --release

FROM --platform=linux/amd64 debian:bookworm-slim
COPY --from=builder /usr/src/app/target/release/backend /usr/local/bin/
CMD ["backend"] 