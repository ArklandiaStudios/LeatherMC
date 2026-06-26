# LeatherMC

A **vanilla Minecraft server written in Rust**, built from scratch — for performance, with
JVM-plugin (`.jar`) compatibility as a long-term goal.

> Status: `0.0.1-alpha` · very early. The server answers the **Server List Ping** and handles
> **offline-mode login** (the client gets past the login screen, then receives a "world coming soon"
> disconnect). Joining an actual world is not implemented yet. Target: Minecraft **26.2** (protocol 776).

## Why

This is a ground-up rewrite. Instead of forking Paper/Spigot (Java), LeatherMC reimplements the
Minecraft server protocol in Rust, one brick at a time, to get native performance. Paper/Bukkit
plugin compatibility (by embedding a JVM via FFI) is planned **after** the vanilla core works.

## Roadmap

The server is built incrementally; each brick depends on the previous one.

1. ✅ **Server List Ping** — the server appears in the multiplayer list.
2. ✅ **Login (offline mode)** — get past the connection screen.
3. Join an empty world — spawn, see the sky. ← *next*
4. Keep-alive + chat.
5. Chunks / ground (flat world).
6. Movement + seeing other players.
7. Break / place blocks.
8. Inventory & items.
9. Entities / mobs.
10. World persistence (Anvil format).
11. World generation.
12. JVM (`.jar`) plugin compatibility.

## Build & run

Requires a recent Rust toolchain (stable).

```bash
cargo run --release --bin leathermc
```

The server listens on `0.0.0.0:25565`. Add `localhost` to your Minecraft multiplayer server list to
see it respond.

## Docker

The release image is built `FROM scratch` with a fully static (musl) binary — minimal and portable
across Linux distributions.

```bash
docker build -t leathermc .
docker run -p 25565:25565 leathermc
```

## Layout

- `crates/protocol` — Minecraft wire-protocol primitives (VarInt, packet framing).
- `crates/server` — the server binary (`leathermc`): networking and connection handling.

## Contributing

Contributions are welcome — the server is built one small brick at a time. Please read
[CONTRIBUTING.md](CONTRIBUTING.md) (workflow, DCO sign-off, coding rules) and our
[Code of Conduct](CODE_OF_CONDUCT.md) first.

## License

[MIT](LICENSE) © Arklandia Studios
