###############################################################################
#                                     Run                                     #
###############################################################################

FROM alpine:latest

LABEL org.opencontainers.image.authors="Adam Talsma <adam@talsma.ca>"

ARG UID=10010
ENV USER=riz
ENV UID=$UID
ENV RIZ_STORAGE_PATH=/data

EXPOSE 8080/tcp

HEALTHCHECK --interval=30s --timeout=1s --start-period=5s --retries=2 CMD [ \
  "wget", \
  "-qO-", \
  "http://localhost:8080/v1/ping" ]

VOLUME /data

RUN adduser \
  --disabled-password \
  --gecos "" \
  --home "/nonexistent" \
  --shell "/sbin/nologin" \
  --no-create-home \
  --uid "${UID}" \
  "${USER}"

COPY ./target/x86_64-unknown-linux-musl/release/riz-api /usr/local/bin/

USER riz:riz

CMD [ "riz-api" ]
