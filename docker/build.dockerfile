###############################################################################
#                                    Build                                    #
###############################################################################

FROM rust:latest

LABEL org.opencontainers.image.authors="Adam Talsma <adam@talsma.ca>"

RUN rustup target add x86_64-unknown-linux-musl
RUN apt-get update && apt-get install -y musl-tools musl-dev
RUN update-ca-certificates

CMD cargo build --target x86_64-unknown-linux-musl --release

###############################################################################
#                             Why Not Multistage?                             #
###############################################################################
#                                                                             #
# We use a separate built container rather than a psuedo-build container      #
# when we want to mount our local cargo registry for faster cached rebuilds   #
#                                                                             #
###############################################################################
