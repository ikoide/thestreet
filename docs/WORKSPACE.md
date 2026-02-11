# The Street Workspace Layout (v0.1)

## Proposed Cargo Workspace

```
thestreet/
  Cargo.toml
  SPEC.md
  PROTOCOL.md
  DATA_MODEL.md
  WORKSPACE.md
  crates/
    common/
      Cargo.toml
      src/
        lib.rs
        config.rs
        ids.rs
        crypto.rs
    protocol/
      Cargo.toml
      src/
        lib.rs
        messages.rs
        signing.rs
    world/
      Cargo.toml
      src/
        lib.rs
        map.rs
        physics.rs
        doors.rs
    wallet/
      Cargo.toml
      src/
        lib.rs
        mock.rs
    relay/
      Cargo.toml
      src/
        main.rs
        server.rs
        state.rs
        storage.rs
    client/
      Cargo.toml
      src/
        main.rs
        ui.rs
        input.rs
        net.rs
        render.rs
  config/
    relay.toml
    client.toml
```

## Crate Responsibilities
- `common`: shared types, ids, config parsing, crypto helpers.
- `protocol`: message types, serialization, signature verification.
- `world`: map generation, physics, door/room mapping.
- `wallet`: wallet trait + mock implementation.
- `relay`: WebSocket server, authoritative state, persistence.
- `client`: TUI, input handling, network, rendering.

## Config Files (MVP)
- `config/relay.toml`:
  - bind addr, onion addr (prod), dev fee config, room price, username fee.
- `config/client.toml`:
  - relay url, socks5 proxy, tor enabled, remote node url (optional).

## Binary Targets
- `relay`: `crates/relay/src/main.rs`
- `client`: `crates/client/src/main.rs`

## Notes
- Keep protocol and world logic independent of UI/IO for testability.
- Mock wallet sits behind a trait to allow real XMR integration later.
