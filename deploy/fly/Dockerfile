FROM rust:1-bookworm AS builder
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq \
    bubblewrap postgresql postgresql-client xfsprogs btrfs-progs util-linux ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /src/target/release/anybranch /usr/local/bin/anybranch
COPY deploy/fly/entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh
ENV PATH="/usr/lib/postgresql/15/bin:/usr/local/bin:${PATH}"
EXPOSE 7432
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
