FROM rust:1 as builder
WORKDIR /build
RUN apt-get --yes update && apt-get --yes install cmake musl-tools
COPY . .

RUN cargo build --release

FROM gcr.io/distroless/static-debian12:nonroot
WORKDIR /app
COPY --from=builder /build/target/release/nb /app/nb
CMD ["/app/nb"]