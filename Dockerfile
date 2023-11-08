####################################################################################################
## Builder
####################################################################################################
FROM rust:1.70.0 as builder

RUN rustup target add x86_64-unknown-linux-musl
RUN apt update && apt install -y musl-tools musl-dev libssl-dev pkg-config
RUN update-ca-certificates

# Create appuser
ENV USER=blockhead
ENV UID=10001
ENV PKG_CONFIG_SYSROOT_DIR=/

RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    "${USER}"


WORKDIR /build

COPY src ./src
COPY Cargo.toml .

RUN cargo build --target x86_64-unknown-linux-musl --release
RUN mkdir app && cp target/x86_64-unknown-linux-musl/release/blockhead app/ 


####################################################################################################
## Final image
####################################################################################################
FROM scratch

# Import from builder.
COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group

# Copy our build
COPY --from=builder --chown=blockhead:blockhead /build/app /app
WORKDIR /app
# Use an unprivileged user.
USER blockhead:blockhead

ENTRYPOINT [ "/app/blockhead" ]

