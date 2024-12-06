FROM --platform=$BUILDPLATFORM rust:1 AS builder

WORKDIR /build
ARG TARGETPLATFORM
RUN \
    set -eux ; \
    if [ "$TARGETPLATFORM" = "linux/amd64" ]; then \
        apt-get --yes update && apt-get --yes install cmake musl-tools ; \
        rustup target add x86_64-unknown-linux-musl ; \
    elif [ "$TARGETPLATFORM" = "linux/arm64" ]; then \
        apt-get --yes update && apt-get --yes install cmake musl-tools gcc-aarch64-linux-gnu ; \
        rustup target add aarch64-unknown-linux-musl ; \
    fi

COPY . .

RUN \
    set -eux ; \
    if [ "$TARGETPLATFORM" = "linux/amd64" ]; then \
        export TARGET=x86_64-unknown-linux-musl ; \
    elif [ "$TARGETPLATFORM" = "linux/arm64" ]; then \
        export TARGET=aarch64-unknown-linux-musl ; \
        export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=aarch64-linux-gnu-gcc ; \
        export CC_aarch64_unknown_linux_musl=aarch64-linux-gnu-gcc ; \
        export CXX_aarch64_unknown_linux_musl=aarch64-linux-gnu-g++ ; \
    fi ; \
    cargo build --release --target ${TARGET} && mkdir -p target/final/release/ && mv target/${TARGET}/release/nb target/final/release/nb ;

RUN file /build/target/final/release/nb

FROM alpine:3
WORKDIR /app
RUN apk add --no-cache git
COPY --from=builder /build/target/final/release/nb /app/nb
COPY ./actions/build/entrypoint.sh /entrypoint.sh
ENTRYPOINT ["/entrypoint.sh"]