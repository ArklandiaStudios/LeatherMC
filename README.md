# LeatherMC

A **vanilla Minecraft server written in Rust**, built from scratch — for performance, with
JVM-plugin (`.jar`) compatibility as a long-term goal.

> Status: `0.0.1-alpha` · very early. A vanilla **26.2** client can **join and walk around an endless
> flat world**: Server List Ping, offline-mode login, Configuration (registries + tags), Play (Join Game,
> spawn) and **chunk streaming** all work. The world is a flat stone floor generated on the fly and sent
> as the player moves. In creative the player can break and place blocks, and edits persist (saved to
> disk and re-sent on reconnect). No entities yet. Target protocol: **776**.

## Why

This is a ground-up rewrite. Instead of forking Paper/Spigot (Java), LeatherMC reimplements the
Minecraft server protocol in Rust, one brick at a time, to get native performance. Paper/Bukkit
plugin compatibility (by embedding a JVM via FFI) is planned **after** the vanilla core works.

## Roadmap

The server is built incrementally; each brick depends on the previous one.

1. ✅ **Server List Ping** — the server appears in the multiplayer list.
2. ✅ **Login (offline mode)** — get past the connection screen.
3. ✅ **Join an empty world** — configuration (registries + tags) + play; spawn, see the sky.
4. ✅ **Chunks / ground** — an endless flat stone world, streamed around the player as they move (keep-alive too).
5. ✅ **Chat** — the player's messages are echoed back as system chat.
6. ✅ **Break / place blocks** — creative: break with left-click, place the held block with right-click.
7. ✅ **World persistence** — edits are kept in a shared world (non-uniform chunks) and saved to disk.
8. **Entities / mobs** — spawn and move entities. ← *next*
9. Inventory & items.
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

## World data (registries)

To **join a world**, the server needs the vanilla registry data (dimension types, biomes, tags, …).
This data is Mojang's and is **not** shipped with LeatherMC, so you generate it once from an official
Minecraft **26.2** server jar with the bundled `leather-datagen` tool:

```bash
# 1. Generate registries.json from the server jar (Mojang's data generator)
java -DbundlerMainClass=net.minecraft.data.Main -jar server.jar --reports

# 2. Convert registries + tags into the NBT files the server loads
cargo run --release --bin leather-datagen -- server.jar ./registries generated/reports/registries.json
```

The server reads the `registries/` directory at startup (configurable). Without it, ping and login
still work, but joining a world does not. The generated data is git-ignored.

## Docker

The release image is built `FROM scratch` with a fully static (musl) binary — minimal and portable
across Linux distributions.

```bash
docker build -t leathermc .
docker run -p 25565:25565 leathermc
```

## Layout

- `crates/protocol` — Minecraft wire-protocol primitives (VarInt, packet framing, NBT).
- `crates/server` — the server binary (`leathermc`): networking and connection handling.
- `crates/datagen` — dev tool that converts a Mojang server jar's registries into the NBT the server serves.

## Contributing

Contributions are welcome — the server is built one small brick at a time. Please read
[CONTRIBUTING.md](CONTRIBUTING.md) (workflow, DCO sign-off, coding rules) and our
[Code of Conduct](CODE_OF_CONDUCT.md) first.

## License

[MIT](LICENSE) © Arklandia Studios
