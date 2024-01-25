FROM rust:1.75.0-slim-bookworm as builder

ENV DEBIAN_FRONTEND noninteractive
RUN apt-get -q update \
	&& apt-get install -y pkg-config libssh-dev
WORKDIR /build/
COPY . /build/
RUN cargo build --release

FROM debian:bookworm-20240110-slim

RUN apt-get -q update \
	&& apt-get install -y \
		openssl \
		ca-certificates \
	&& apt-get -y clean \
	&& rm -rf /var/lib/apt/lists/*
ARG REGSKIN_LOG_LEVEL
ENV REGSKIN_LOG_LEVEL=${REGSKIN_LOG_LEVEL:-info}
ARG REGSKIN_LISTEN
ENV REGSKIN_LISTEN=${REGSKIN_LISTEN:-0.0.0.0}
RUN mkdir /opt/regskin
WORKDIR /opt/regskin
COPY --from=builder /build/target/release/regskin .
COPY --from=builder /build/static static
CMD ["/opt/regskin/regskin"]
