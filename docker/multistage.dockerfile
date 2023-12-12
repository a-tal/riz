###############################################################################
#                                    Build                                    #
###############################################################################
FROM rust:latest AS build

RUN rustup target add x86_64-unknown-linux-musl
RUN apt-get clean && apt-get update && apt-get install -y musl-tools musl-dev
RUN update-ca-certificates

ARG UID=10010
ENV USER=riz
ENV UID=$UID

RUN adduser \
  --disabled-password \
  --gecos "" \
  --home "/nonexistent" \
  --shell "/sbin/nologin" \
  --no-create-home \
  --uid "${UID}" \
  "${USER}"

WORKDIR /riz

COPY ./ .

RUN cargo build --target x86_64-unknown-linux-musl --release


###############################################################################
#                                     Run                                     #
###############################################################################
FROM alpine:latest

LABEL org.opencontainers.image.authors="Adam Talsma <adam@talsma.ca>"

ENV RIZ_STORAGE_PATH=/data

EXPOSE 8080/tcp

VOLUME /data

HEALTHCHECK --interval=30s --timeout=1s --start-period=5s --retries=2 CMD [ \
  "wget", \
  "-qO-", \
  "http://localhost:8080/v1/ping" ]

COPY --from=build /etc/passwd /etc/passwd
COPY --from=build /etc/group /etc/group
COPY --from=build /riz/target/x86_64-unknown-linux-musl/release/riz-api /usr/local/bin/

USER riz:riz

CMD [ "riz-api" ]
