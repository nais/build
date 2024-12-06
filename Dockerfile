FROM rust:1 as builder
WORKDIR /build
RUN apt-get --yes update && apt-get --yes install cmake musl-tools
COPY . .

RUN rustup target add x86_64-unknown-linux-musl
RUN cargo build --release --target x86_64-unknown-linux-musl --target-dir /output

FROM gcr.io/distroless/static-debian12:nonroot
WORKDIR /app
COPY --from=builder /output/x86_64-unknown-linux-musl/release/nb /app/nb
CMD ["/app/nb", "preflight"]