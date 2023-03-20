####################################################################################################
## Builder
####################################################################################################
FROM rust:1.67 as builder

RUN rustup target add x86_64-unknown-linux-musl
RUN apt update && apt install -y musl-tools musl-dev libssl-dev pkg-config
RUN update-ca-certificates

# Create appuser
ENV USER=injector
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


WORKDIR /injector

COPY ./ .

RUN cargo build --target x86_64-unknown-linux-musl --release

####################################################################################################
## Final image
####################################################################################################
FROM scratch

# Import from builder.
COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group

WORKDIR /injector

# Copy our build
COPY --from=builder /injector/target/x86_64-unknown-linux-musl/release/injector ./

# Use an unprivileged user.
USER injector:injector

CMD ["/injector/injector"]
