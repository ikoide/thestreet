# The Street MVP Spec Sheet (v0.1)

## Vision
- Terminal-first social world inspired by Snow Crash.
- Single authoritative relay server (no federation).
- Rust MVP: WebSocket relay + TUI client.
- Tor/SOCKS5 first; clearnet optional.

## World Model
### Street
- ASCII-rendered, height = 16 rows including top and bottom wall rows.
- Top and bottom boundaries are `#` walls.
- Doors appear every 6 columns (5 chars between doors); top/bottom patterns are offset.
- Infinite length via segment rendering: client draws as much width as the terminal allows.
- When player crosses the right edge, render the next segment.
- Street x coordinates are unbounded (negative and positive).
- Client shows ring position as a percent around the circle and the offset to the nearest block.

### Monorail
- Two-row track runs through the center of the Street.
- Four trains circle the ring continuously at high speed.
- Stations at x=0 (spawn) and x=circumference/2 (opposite).
- Station entrance doors are marked with `M` on the Street.
- Boarding happens when a train arrives; trains do not stop.

### Rooms
- Separate maps with their own ASCII layout.
- Room size in MVP: 8 rows high, 32 columns wide.
- Always include an exit door that returns to the street.
- Door access: open, whitelist, or blacklist, configurable by owner.
- Rooms follow the same base physics as the street.
- Rooms include a customizer object `C` (impassable) used to set room name and door color.

## Client UI (TUI)
- Top-left: location label (Street or Room name).
- Top-right: ring percent and offset to nearest block.
- Top area: street/room renderer.
- Lower area: info panel, chat log, input field.

## Movement + Physics
- Grid-based movement.
- Collision against `#` walls.
- Door traversal happens when stepping into a door tile (no command).

## Chat + Messaging
### Local chat
- Use `/say <message>`.
- Street proximity: 8x8 grid centered on speaker (Chebyshev distance).

### Whisper
- `/whisper <message>` sends to a 3x3 grid centered on speaker.

### Room chat
- Visible to everyone in the same room.

## Commands (MVP)
- `/say <message>`: local chat.
- `/who`:
  - Street: users within 8x8 grid.
  - Room: all users in the room.
- `/whisper <message>`: 3x3 grid centered on speaker.
- `/buy`: purchase current room/parcel.
- `/pay <user> <amount>`: send XMR (placeholder integration).
- `/balance`: show balance (placeholder integration).
- `/faucet [amount]`: dev credit (placeholder).
- `/board <spawn|opposite>`: board monorail from station.
- `/depart <spawn|opposite>`: set monorail destination.
- `/room_name <name>`: set room name (owner only).
- `/door_color <color>`: set room door color (owner only).
- `/claim_name <name>`: purchase a unique display name.
- `/access <open|whitelist|blacklist> [user|pubkey...]`: set room access policy.
- `/access show`: show current room access list.
- `/help`: show command list and usage.
- Client may parse for UX; server has final authority.

## Identity + Sessions
- Server assigns a unique client identifier on first connection.
- User can set a unique display name by paying 0.1 XMR (MVP).
- Anonymous sessions with persistent identity via keypair:
  - Client generates and stores a keypair (ed25519 suggested).
  - Server binds user state to the public key.
  - Client can restore by pasting/importing the key.
  - Authenticated actions are signed by the client.

## Rooms + Ownership
- Ownership metadata includes: owner id/name, price, for-sale status, access policy.
- Info panel displays owner, price, and sale status.

## Economy (Placeholder-First)
- XMR custody is client-side only.
- Server applies a developer fee on transfers/sales:
  - Fee is dynamic and configurable server-side.
- Room price in MVP: 1 XMR.
- Room purchases require 8 confirmations.
- MVP uses mock balances and transaction IDs behind a wallet interface.

## Networking
- WebSocket relay server.
- Tor/SOCKS5 default on client; disabling Tor allowed for dev.
- Production relay exposes an onion service; dev uses localhost.

## Security Posture (MVP)
- Signed client messages using client keypair.
- Server rate-limits to prevent chat spam.
- Server never stores client private keys.
- XMR integration isolated behind a wallet interface to allow real integration later.

## Supporting Docs
- Protocol details: `PROTOCOL.md`
- Data model: `DATA_MODEL.md`
- Workspace layout: `WORKSPACE.md`

## Non-Goals (MVP)
- Federation or multi-server routing.
- Escrow or payment requests.
- Full Monero node requirement on clients.
