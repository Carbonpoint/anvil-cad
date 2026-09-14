# Anvil CAD in a container: the headless command-line tools, no GUI.
#
# Build:   docker build -t anvil-cad .
# Card:    docker run --rm -v "$PWD/out:/out" anvil-cad card --name "Your Name" --url "https://www.linkedin.com/in/your-handle"
# Tests:   docker build --target test .
#
# The desktop app needs a display and is not part of this image.

ARG RUST_VERSION=1.88

# ---- build: compile the command-line tool --------------------------------
FROM rust:${RUST_VERSION}-slim-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release --locked -p anvil-cli

# ---- test: run the tests of every crate that needs no display ------------
FROM build AS test
RUN cargo test --locked \
    -p anvil-math -p anvil-expr -p anvil-sketch -p anvil-kernel \
    -p anvil-feature -p anvil-cam -p anvil-io -p anvil-cli

# ---- runtime: small image with just the binary ---------------------------
FROM debian:bookworm-slim AS runtime
RUN useradd --create-home --uid 1000 anvil
COPY --from=build /src/target/release/anvil-cli /usr/local/bin/anvil-cli
WORKDIR /out
USER anvil
ENTRYPOINT ["anvil-cli"]
CMD ["--help"]
