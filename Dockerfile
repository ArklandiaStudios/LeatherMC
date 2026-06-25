# --- Build stage: compile a fully static (musl) binary ----------------------
# rust:alpine targets musl by default, so the resulting binary has no dynamic
# dependencies and can run on virtually any Linux distribution.
FROM rust:alpine AS builder

RUN apk add --no-cache musl-dev

WORKDIR /app
COPY . .
RUN cargo build --release --bin leathermc

# --- Runtime stage: empty image + the static binary -------------------------
# `scratch` is a zero-byte base; the image is just our binary. Minimal and
# portable, at the cost of having no shell for in-container debugging.
FROM scratch

COPY --from=builder /app/target/release/leathermc /leathermc

EXPOSE 25565
ENTRYPOINT ["/leathermc"]
