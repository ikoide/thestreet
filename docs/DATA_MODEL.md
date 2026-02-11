# The Street Data Model (v0.1)

## Coordinate System
- All positions are integer grid coordinates.
- Street map id is `street`.
- Room map ids are `room/<room_id>`.
- Station map ids are `station/<station_x>`.
- Train map ids are `train/<train_id>`.

### Street
- Height: 16 rows (y = 0..15).
- Top/bottom walls: y = 0 and y = 15.
- Walkable rows: y = 1..14.
- Infinite length along x (x is unbounded, negative and positive).

### Rooms (MVP)
- Size: 32 columns wide (x = 0..31), 16 rows high (y = 0..15).
- Top/bottom walls: y = 0 and y = 15.
- Left/right walls: x = 0 and x = 31.
- Walkable interior: x = 1..30, y = 1..14.
- Customizer object `C` at x=31, y=1 (impassable).

### Stations (MVP)
- Size: 32 columns wide (x = 0..31), 16 rows high (y = 0..15).
- Exit door at bottom center back to the Street.

### Trains (MVP)
- Size: 32 columns wide (x = 0..31), 16 rows high (y = 0..15).
- No fixed door; disembark is command-driven when passing stations.

## Doors
- Doors are single tiles on the street wall rows (top/bottom).
- Top wall doors when `x % 6 == 0` (5 chars between doors).
- Bottom wall doors when `x % 6 == 3` (offset pattern).
- Door tiles are represented as `D`.

## Room Identity
- Room id is deterministic from street side and x coordinate:
  - `room_id = "north:<x>"` for top wall doors.
  - `room_id = "south:<x>"` for bottom wall doors.
- Map id is `room/<room_id>`.

## Core Entities

### User
```json
{
  "user_id": "u_123",
  "pubkey": "base64-ed25519-pubkey",
  "display_name": null,
  "position": {"map_id": "street", "x": 12, "y": 3},
  "last_seen": 0
}
```

### Room
```json
{
  "room_id": "north:42",
  "owner_pubkey": "base64-ed25519-pubkey",
  "price_xmr": "1.0",
  "for_sale": true,
  "access": {"mode": "open", "list": []},
  "width": 32,
  "height": 8
}
```

### Access Policy
```json
{
  "mode": "open|whitelist|blacklist",
  "list": ["base64-ed25519-pubkey"]
}
```

### Transaction (Placeholder)
```json
{
  "tx_id": "mock",
  "from_pubkey": "...",
  "to_pubkey": "...",
  "amount_xmr": "1.0",
  "fee_xmr": "0.01",
  "status": "pending|confirmed|failed",
  "confirmations": 0
}
```

### Developer Fee Config
```json
{
  "mode": "bps|percent",
  "value": 100
}
```

## Derived World State
- Street geometry is deterministic; no persistence required.
- Room definitions and ownership persist on the server.
- User profiles persist by public key.

## Persistence (MVP)
- Store in a single local database or JSON file per type:
  - `users.json`
  - `rooms.json`
  - `transactions.json` (placeholder only)

## Wallet Interface (Placeholder)
- The server interacts with a wallet interface that can be swapped for real XMR integration.
- Minimal interface:
  - `get_balance(pubkey)`
  - `send(from_pubkey, to_pubkey, amount, fee)`
  - `get_tx_status(tx_id)`
