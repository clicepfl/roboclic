# Setup runtime image
FROM debian:bookworm AS runtime
RUN apt-get update && apt-get upgrade -y && apt-get install -y ca-certificates openssl
#RUN ln -s /usr/lib/x86_64-linux-gnu/libssl.so.1.1 /usr/lib/libssl.so.3
#RUN ln -s /usr/lib/x86_64-linux-gnu/libcrypto.so /usr/lib/libcrypto.so.3

# Build on a dedicated image to avoid build output in final image
FROM rust:latest AS rust_build
# cargo-build-deps is a tool to only install and build dependencies
RUN cargo install cargo-build-deps
RUN cargo new app
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN cargo build-deps --release
# Add and build project
COPY src ./src
RUN cargo build --release

# Copies build result into runtime image
FROM runtime
WORKDIR /app
COPY --from=rust_build /app/target/release/roboclic-v2 ./roboclic

ENV RUST_LOG=info

CMD ["./roboclic"]
