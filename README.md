# The Street MVP

## Quick start (dev)

1. Start the relay:

```bash
cargo run -p street-relay -- --config config/relay.toml
```

2. Start the client:

```bash
cargo run -p street-client -- --config config/client.toml
```

```bash
cargo run -p street-client -- --config config/client2.toml
```

## Config

- Relay config: `config/relay.toml`
- Client config: `config/client.toml`
- Client identity key: `config/identity.key` (auto-generated)

## Multiple local clients

- Each client needs a unique `identity_key_path`.
- Use the included `config/client2.toml` for a second local client.

## Controls

- Move: arrow keys or WASD
- Input: press `/` to enter command mode
- Info: press `i` to cycle info panels
- Chat: `/say <msg>` or `/whisper <msg>`
- Commands: `/say <msg>`, `/who`, `/whisper <msg>`, `/buy`, `/pay <user> <amount>`, `/balance`, `/faucet [amount]`, `/board <spawn|opposite>`, `/depart <spawn|opposite>`, `/room_name <name>`, `/door_color <color>`, `/claim_name <name>`, `/access <open|whitelist|blacklist> [user|pubkey...]`, `/access show`, `/exit`, `/help`
- Quit: Esc or Ctrl+Q

## Monorail

- Track runs through the center of the Street.
- Station doors are marked with `M` at spawn and opposite.
- Enter a station by stepping on `M`, then board with `1/2` or `/board`.

## Notes

- Tor/SOCKS5 is supported via `socks5_proxy` when `tor_enabled = true`.
- XMR is mocked in MVP; wallet integration is stubbed behind an interface.
