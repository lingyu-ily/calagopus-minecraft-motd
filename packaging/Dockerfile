FROM rust:1-bookworm AS builder

WORKDIR /build
COPY agent/Cargo.toml agent/Cargo.lock ./
COPY agent/src ./src
RUN cargo build --release --locked

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        iptables \
        libgcc-s1 \
        nftables \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/calagopus-minecraft-motd-agent \
    /usr/local/bin/calagopus-minecraft-motd-agent

ENTRYPOINT ["/usr/local/bin/calagopus-minecraft-motd-agent"]
CMD ["run", "--config", "/etc/calagopus-motd-agent/config.toml"]
