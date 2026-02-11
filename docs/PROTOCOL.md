# The Street Protocol (v0.1)

## Transport
- WebSocket over TCP.
- Payloads are JSON objects encoded as UTF-8.
- One JSON message per WebSocket frame.
- Max message size (MVP): 64 KiB.

## Envelope
All messages use a common envelope:

```json
{
  "type": "client.chat",
  "id": "uuid-string",
  "ts": 1700000000000,
  "sig": "base64-signature-optional",
  "payload": {"text": "hello"}
}
```

- `type`: message kind, namespaced as `client.*` or `server.*`.
- `id`: unique message id for tracing and de-dupe.
- `ts`: unix epoch milliseconds.
- `sig`: optional signature; required for authenticated actions.
- `payload`: message body.

## Signing
- Algorithm: ed25519.
- `sig` is required for authenticated actions (movement, chat, commands, room access updates).
- Signing bytes are canonical JSON (RFC 8785 JCS) of:

```json
{
  "type": "...",
  "id": "...",
  "ts": 0,
  "payload": {...}
}
```

- `sig` is base64 of the ed25519 signature.

## Connection + Auth Flow
1. `server.hello` -> includes challenge nonce and config.
2. `client.auth` -> includes public key and signature of challenge.
3. `server.welcome` -> assigns `client_id` and initial state.

### server.hello
```json
{
  "type": "server.hello",
  "payload": {
    "server_version": "0.1",
    "challenge": "base64-nonce",
    "fee_config": {"mode": "bps", "value": 100},
    "room_price_xmr": "1.0",
    "username_fee_xmr": "0.1"
  }
}
```

### client.auth
```json
{
  "type": "client.auth",
  "payload": {
    "pubkey": "base64-ed25519-pubkey",
    "challenge_sig": "base64-signature",
    "client_version": "0.1"
  }
}
```

### server.welcome
```json
{
  "type": "server.welcome",
  "payload": {
    "client_id": "u_123",
    "display_name": null,
    "position": {"map_id": "street", "x": 12, "y": 3},
    "session_id": "s_456"
  }
}
```

## Client -> Server Messages

### client.move
- Payload: `{ "dir": "up|down|left|right" }`
- Server validates collisions and map transitions.

### client.chat
- Payload: `{ "scope": "local|whisper|room", "text": "..." }`
- `scope` defaults to `local` if omitted.

### client.command
- Payload: `{ "name": "who|buy|pay|balance|faucet|board|depart|room_name|door_color|claim_name|access|help|room_info", "args": ["..."] }`
- Server is authoritative; client may pre-parse input for UX.

### client.room_access_update
- Payload: `{ "room_id": "...", "mode": "open|whitelist|blacklist", "list": ["pubkey..."] }`
- Only room owner may update.

### client.heartbeat
- Payload: `{ "nonce": "..." }`

## Server -> Client Messages

### server.state
- Payload: `{ "position": {"map_id": "...", "x": 0, "y": 0} }`

### server.map_change
- Payload: `{ "map_id": "...", "position": {"x": 0, "y": 0} }`

### server.chat
- Payload: `{ "from": "u_123", "display_name": "...", "text": "...", "scope": "..." }`

### server.nearby
- Payload: `{ "users": [{"id": "u_123", "display_name": "...", "x": 0, "y": 0}] }`

### server.train_state
- Payload: `{ "trains": [{"id": 0, "x": 1234.5}] }`

### server.who
- Payload: `{ "users": [{"id": "u_123", "display_name": "..."}] }`

### server.room_info
- Payload: `{ "room_id": "...", "owner": "...", "price_xmr": "1.0", "for_sale": true, "access": {"mode": "open", "list": []} }`

### server.tx_update (placeholder)
- Payload: `{ "tx_id": "mock", "status": "pending|confirmed|failed", "confirmations": 0 }`

### server.error
- Payload: `{ "code": "...", "message": "..." }`

### server.notice
- Payload: `{ "text": "..." }`

### server.heartbeat
- Payload: `{ "nonce": "..." }`

## Error Codes (MVP)
- `auth_failed`
- `already_connected`
- `invalid_signature`
- `rate_limited`
- `move_blocked`
- `room_access_denied`
- `insufficient_funds`
- `wallet_error`
- `invalid_command`

## Notes
- Server should accept both `client.command` and `client.chat` with a `/` prefix for compatibility with simple clients.
- Client should treat all server state as authoritative.
