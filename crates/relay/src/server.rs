use std::sync::Arc;

use base64::{engine::general_purpose, Engine as _};
use futures_util::{SinkExt, StreamExt};
use rand::RngCore;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

use street_common::config::RelayConfig;
use street_common::crypto::{decode_verifying_key, verify_signature};
use street_common::ids::new_message_id;
use street_protocol::signing::unsigned_envelope;
use street_protocol::{
    AccessMode, AccessPolicy, ChatScope, ClientAuth, ClientChat, ClientCommand, ClientMove,
    ClientRoomAccessUpdate, ClientRoomKey, DevFeeConfig as WireDevFee, Direction as WireDirection,
    Envelope, NearbyUser, ServerChat, ServerError, ServerHello, ServerMapChange, ServerNearby,
    ServerNotice, ServerRoomInfo, ServerRoomKey, ServerState, ServerTrainState, ServerTxUpdate,
    ServerWelcome, ServerWho, TrainInfo, WhoUser,
};
use street_wallet::mock::MockWallet;
use street_wallet::Wallet;
use street_world::{
    parse_room_id, parse_room_map_id, room_customizer_position, room_id_for_door, try_move,
    Direction, MoveOutcome, STREET_CIRCUMFERENCE_TILES, TRAIN_HEIGHT, TRAIN_WIDTH,
};
use street_world::monorail::{
    parse_station_map_id, parse_train_map_id, station_positions, station_x_for_label, train_map_id,
    STATION_DOOR_Y_BOTTOM, STATION_DOOR_Y_TOP,
};

use crate::state::{
    BoardingRequest, ClientHandle, RoomState, ServerState as RelayState, TrainRide, TrainState,
    UserState,
};
use crate::storage::Storage;

const LOCAL_CHAT_WIDTH: i32 = 16;
const LOCAL_CHAT_HEIGHT: i32 = 16;
const WHISPER_WIDTH: i32 = 5;
const WHISPER_HEIGHT: i32 = 5;
const MOVE_INTERVAL_MS: i64 = 60;
const TRAIN_STATE_BROADCAST_TICKS: u32 = 2;

pub struct RelayServer {
    config: RelayConfig,
    state: Arc<RwLock<RelayState>>,
    storage: Arc<Storage>,
    wallet: MockWallet,
}

impl RelayServer {
    pub fn new(config: RelayConfig, storage: Storage, state: RelayState, wallet: MockWallet) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(state)),
            storage: Arc::new(storage),
            wallet,
        }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let listener = TcpListener::bind(&self.config.bind_addr).await?;
        log_info(&format!("relay listening on {}", self.config.bind_addr));
        spawn_train_loop(Arc::clone(&self.state));
        loop {
            let (stream, addr) = listener.accept().await?;
            let conn_id = uuid::Uuid::new_v4().to_string();
            log_info(&format!("conn {conn_id} accepted from {addr}"));
            let state = Arc::clone(&self.state);
            let storage = Arc::clone(&self.storage);
            let config = self.config.clone();
            let wallet = self.wallet.clone();
            tokio::spawn(async move {
                if let Err(err) = handle_connection(stream, state, storage, wallet, config, conn_id.clone(), addr).await {
                    log_error(&format!("conn {conn_id} error: {err}"));
                }
            });
        }
    }
}

async fn handle_connection(
    stream: tokio::net::TcpStream,
    state: Arc<RwLock<RelayState>>,
    storage: Arc<Storage>,
    wallet: MockWallet,
    config: RelayConfig,
    conn_id: String,
    addr: std::net::SocketAddr,
) -> anyhow::Result<()> {
    let ws_stream = accept_async(stream).await?;
    let (mut ws_write, mut ws_read) = ws_stream.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    let writer = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_write.send(msg).await.is_err() {
                break;
            }
        }
    });

    let challenge = random_challenge();
    let hello = ServerHello {
        server_version: "0.1".to_string(),
        challenge: challenge.clone(),
        fee_config: WireDevFee {
            mode: config.dev_fee.mode.to_string(),
            value: config.dev_fee.value,
        },
        room_price_xmr: config.room_price_xmr.clone(),
        username_fee_xmr: config.username_fee_xmr.clone(),
    };
    send_envelope(&tx, "server.hello", &hello)?;

    let mut authed_user: Option<UserState> = None;
    let mut verifying_key = None;
    let mut user_label: Option<String> = None;

    while let Some(msg) = ws_read.next().await {
        let msg = msg?;
        if !msg.is_text() {
            continue;
        }
        let env: Envelope = serde_json::from_str(msg.to_text()?)?;

        if authed_user.is_none() {
            if env.message_type != "client.auth" {
                send_error(&tx, "auth_failed", "expected client.auth")?;
                log_warn(&format!("conn {conn_id} auth failed: expected client.auth"));
                break;
            }
            let auth: ClientAuth = serde_json::from_value(env.payload)?;
            let verifying = decode_verifying_key(&auth.pubkey)?;
            if !verify_challenge(&verifying, &challenge, &auth.challenge_sig)? {
                send_error(&tx, "auth_failed", "invalid challenge signature")?;
                log_warn(&format!("conn {conn_id} auth failed: invalid challenge signature"));
                break;
            }

            let mut state_guard = state.write().await;
            let mut user = if let Some(user_id) = state_guard.users_by_pubkey.get(&auth.pubkey).cloned() {
                state_guard.users.get(&user_id).cloned().unwrap_or_else(|| {
                    state_guard.create_user(auth.pubkey.clone())
                })
            } else {
                let user = state_guard.create_user(auth.pubkey.clone());
                wallet.credit(&auth.pubkey, 10.0);
                user
            };

            if let Some(x25519_pubkey) = &auth.x25519_pubkey {
                user.x25519_pubkey = Some(x25519_pubkey.clone());
                if let Some(entry) = state_guard.users.get_mut(&user.user_id) {
                    entry.x25519_pubkey = Some(x25519_pubkey.clone());
                }
            }

            if state_guard.clients.contains_key(&user.user_id) {
                drop(state_guard);
                send_error(&tx, "already_connected", "user already connected")?;
                log_warn(&format!("conn {conn_id} user {} already connected", user.user_id));
                break;
            }

            let session_id = uuid::Uuid::new_v4().to_string();
            state_guard.clients.insert(
                user.user_id.clone(),
                ClientHandle {
                    user_id: user.user_id.clone(),
                    pubkey: user.pubkey.clone(),
                    tx: tx.clone(),
                },
            );
            state_guard.add_connected_user(&user.user_id, &user.position.map_id);

            let welcome = ServerWelcome {
                client_id: user.user_id.clone(),
                display_name: user.display_name.clone(),
                position: street_protocol::Position {
                    map_id: user.position.map_id.clone(),
                    x: user.position.x,
                    y: user.position.y,
                },
                session_id,
            };
            send_envelope(&tx, "server.welcome", &welcome)?;

            send_train_state(&tx, &state_guard.trains)?;

            user_label = Some(user.user_id.clone());
            log_info(&format!("conn {conn_id} authenticated as {} ({})", user.user_id, auth.pubkey));
            
            refresh_nearby_for_map(&state_guard, &user.position.map_id)?;

            authed_user = Some(user);
            verifying_key = Some(verifying);
            continue;
        }

        let user = authed_user.as_mut().expect("user set");
        if let Some(current) = state.read().await.users.get(&user.user_id).cloned() {
            *user = current;
        }
        let verifying = verifying_key.as_ref().expect("verifying set");

        if requires_signature(&env.message_type) && !verify_signed_envelope(&env, verifying)? {
            send_error(&tx, "invalid_signature", "signature required")?;
            if let Some(label) = &user_label {
                log_warn(&format!("{label} invalid signature for {}", env.message_type));
            } else {
                log_warn(&format!("conn {conn_id} invalid signature for {}", env.message_type));
            }
            continue;
        }

        match env.message_type.as_str() {
            "client.move" => {
                let payload: ClientMove = serde_json::from_value(env.payload)?;
                handle_move(payload, user, &state, &storage, &config, &tx).await?;
            }
            "client.chat" => {
                let payload: ClientChat = serde_json::from_value(env.payload)?;
                if let Some(label) = &user_label {
                    log_info(&format!("{label} chat scope={:?} len={}", payload.scope, payload.text.len()));
                }
                handle_chat(payload, user, &state, &tx).await?;
            }
            "client.command" => {
                let payload: ClientCommand = serde_json::from_value(env.payload)?;
                if let Some(label) = &user_label {
                    log_info(&format!("{label} command {} {:?}", payload.name, payload.args));
                }
                handle_command(payload, user, &state, &storage, &wallet, &config, &tx).await?;
            }
            "client.room_access_update" => {
                let payload: ClientRoomAccessUpdate = serde_json::from_value(env.payload)?;
                if let Some(label) = &user_label {
                    log_info(&format!("{label} room access update {}", payload.room_id));
                }
                handle_room_access(payload, user, &state, &storage, &config, &tx).await?;
            }
            "client.room_key" => {
                let payload: ClientRoomKey = serde_json::from_value(env.payload)?;
                if let Some(label) = &user_label {
                    log_info(&format!("{label} room key {} -> {}", payload.room_id, payload.target));
                }
                handle_room_key(payload, user, &state, &tx).await?;
            }
            "client.heartbeat" => {
                send_notice(&tx, "pong")?;
            }
            _ => {
                send_error(&tx, "invalid_command", "unknown message type")?;
                if let Some(label) = &user_label {
                    log_warn(&format!("{label} unknown message type {}", env.message_type));
                }
            }
        }
    }

    if let Some(user) = authed_user {
        let map_id = user.position.map_id.clone();
        let mut state_guard = state.write().await;
        state_guard.clients.remove(&user.user_id);
        state_guard.remove_connected_user(&user.user_id, &map_id);
        state_guard.boarding.remove(&user.user_id);
        state_guard.riders.remove(&user.user_id);
        refresh_nearby_for_map(&state_guard, &map_id)?;
        let users = state_guard.users.values().cloned().collect::<Vec<_>>();
        let rooms = state_guard.rooms.values().cloned().collect::<Vec<_>>();
        drop(state_guard);
        storage.save_users_async(users).await?;
        storage.save_rooms_async(rooms).await?;
        log_info(&format!("user {} disconnected", user.user_id));
    } else {
        log_info(&format!("conn {conn_id} from {addr} closed before auth"));
    }

    writer.abort();
    Ok(())
}

fn random_challenge() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    general_purpose::STANDARD.encode(bytes)
}

fn verify_challenge(
    verifying: &ed25519_dalek::VerifyingKey,
    challenge: &str,
    signature_b64: &str,
) -> anyhow::Result<bool> {
    Ok(verify_signature(verifying, challenge.as_bytes(), signature_b64))
}

fn requires_signature(message_type: &str) -> bool {
    match message_type {
        "client.auth" | "client.heartbeat" => false,
        _ if message_type.starts_with("client.") => true,
        _ => false,
    }
}

fn verify_signed_envelope(
    envelope: &Envelope,
    verifying: &ed25519_dalek::VerifyingKey,
) -> anyhow::Result<bool> {
    use street_protocol::signing::verify_envelope;
    verify_envelope(envelope, verifying)
}

fn send_envelope<T: serde::Serialize>(
    tx: &mpsc::UnboundedSender<Message>,
    message_type: &str,
    payload: &T,
) -> anyhow::Result<()> {
    let env = unsigned_envelope(message_type, &new_message_id(), now_ms(), payload)?;
    let text = serde_json::to_string(&env)?;
    tx.send(Message::Text(text))?;
    Ok(())
}

fn send_error(tx: &mpsc::UnboundedSender<Message>, code: &str, message: &str) -> anyhow::Result<()> {
    let payload = ServerError {
        code: code.to_string(),
        message: message.to_string(),
    };
    send_envelope(tx, "server.error", &payload)
}

fn send_notice(tx: &mpsc::UnboundedSender<Message>, text: &str) -> anyhow::Result<()> {
    let payload = ServerNotice {
        text: text.to_string(),
    };
    send_envelope(tx, "server.notice", &payload)
}

async fn handle_move(
    payload: ClientMove,
    user: &mut UserState,
    state: &Arc<RwLock<RelayState>>,
    storage: &Arc<Storage>,
    config: &RelayConfig,
    tx: &mpsc::UnboundedSender<Message>,
) -> anyhow::Result<()> {
    let now = now_ms();
    {
        let mut state_guard = state.write().await;
        let last = state_guard
            .last_move_ms
            .get(&user.user_id)
            .copied()
            .unwrap_or(0);
        if now - last < MOVE_INTERVAL_MS {
            return Ok(());
        }
        state_guard.last_move_ms.insert(user.user_id.clone(), now);
    }

    let prev_map = user.position.map_id.clone();
    let dir = match payload.dir {
        WireDirection::Up => Direction::Up,
        WireDirection::Down => Direction::Down,
        WireDirection::Left => Direction::Left,
        WireDirection::Right => Direction::Right,
    };

    let outcome = try_move(&user.position, dir);
    match outcome {
        MoveOutcome::Blocked => {
            send_error(tx, "move_blocked", "blocked")?;
        }
        MoveOutcome::Moved(pos) => {
            user.position = pos.clone();
            let mut state_guard = state.write().await;
            if let Some(entry) = state_guard.users.get_mut(&user.user_id) {
                entry.position = pos.clone();
            }
            let payload = ServerState {
                position: street_protocol::Position {
                    map_id: pos.map_id,
                    x: pos.x,
                    y: pos.y,
                },
            };
            send_envelope(tx, "server.state", &payload)?;
            refresh_nearby_for_map(&state_guard, &user.position.map_id)?;
        }
        MoveOutcome::Transition(pos) => {
            let from_map = user.position.map_id.clone();
            if let Some((side, street_x)) = parse_room_map_id(&pos.map_id) {
                let room_id = room_id_for_door(side, street_x);
                let mut state_guard = state.write().await;
                let room = state_guard.get_or_create_room(&room_id, &config.room_price_xmr);
                if !room_access_allowed(&room, &user.pubkey) {
                    send_error(tx, "room_access_denied", "room access denied")?;
                    return Ok(());
                }
                user.position = pos.clone();
                if let Some(entry) = state_guard.users.get_mut(&user.user_id) {
                    entry.position = pos.clone();
                }
                if !user.position.map_id.starts_with("station/") {
                    state_guard.boarding.remove(&user.user_id);
                }
                state_guard.move_connected_user(&user.user_id, &prev_map, &user.position.map_id);
                let rooms_snapshot = state_guard.rooms.values().cloned().collect::<Vec<_>>();
                let payload = ServerMapChange {
                    map_id: pos.map_id.clone(),
                    position: street_protocol::Position {
                        map_id: pos.map_id.clone(),
                        x: pos.x,
                        y: pos.y,
                    },
                };
                send_envelope(tx, "server.map_change", &payload)?;
                send_room_info(tx, &room)?;
                refresh_nearby_for_map(&state_guard, &prev_map)?;
                refresh_nearby_for_map(&state_guard, &user.position.map_id)?;
                log_info(&format!("user {} moved {} -> {}", user.user_id, from_map, user.position.map_id));
                drop(state_guard);
                storage.save_rooms_async(rooms_snapshot).await?;
            } else {
                user.position = pos.clone();
                let mut state_guard = state.write().await;
                if let Some(entry) = state_guard.users.get_mut(&user.user_id) {
                    entry.position = pos.clone();
                }
                if !user.position.map_id.starts_with("station/") {
                    state_guard.boarding.remove(&user.user_id);
                }
                state_guard.move_connected_user(&user.user_id, &prev_map, &user.position.map_id);
                let payload = ServerMapChange {
                    map_id: pos.map_id.clone(),
                    position: street_protocol::Position {
                        map_id: pos.map_id.clone(),
                        x: pos.x,
                        y: pos.y,
                    },
                };
                send_envelope(tx, "server.map_change", &payload)?;
                refresh_nearby_for_map(&state_guard, &prev_map)?;
                refresh_nearby_for_map(&state_guard, &user.position.map_id)?;
                log_info(&format!("user {} moved {} -> {}", user.user_id, from_map, user.position.map_id));
            }
        }
    }
    Ok(())
}

async fn handle_chat(
    payload: ClientChat,
    user: &UserState,
    state: &Arc<RwLock<RelayState>>,
    tx: &mpsc::UnboundedSender<Message>,
) -> anyhow::Result<()> {
    let scope = payload.scope.unwrap_or(ChatScope::Local);
    let text = payload.text;
    let enc = payload.enc.clone();
    let room_id = user.position.map_id.strip_prefix("room/").map(|value| value.to_string());
    let mut recipients = Vec::new();

    let state_guard = state.read().await;
    let users_in_map = connected_users_in_map(&state_guard, &user.position.map_id);
    match scope {
        ChatScope::Whisper => {
            let Some(enc) = &enc else {
                send_error(tx, "invalid_command", "whisper must be encrypted")?;
                return Ok(());
            };
            if enc.sender_key.is_none() {
                send_error(tx, "invalid_command", "missing whisper sender key")?;
                return Ok(());
            }
            let Some(target) = payload.target.as_deref() else {
                send_error(tx, "invalid_command", "usage: /whisper <user> <msg>")?;
                return Ok(());
            };
            let target_user = users_in_map
                .iter()
                .find(|u| u.user_id == target || u.display_name.as_deref() == Some(target));
            let target_user = match target_user {
                Some(user) => user,
                None => {
                    send_error(tx, "invalid_command", "whisper target not found")?;
                    return Ok(());
                }
            };
            if !in_box(
                user.position.x,
                user.position.y,
                target_user.position.x,
                target_user.position.y,
                WHISPER_WIDTH,
                WHISPER_HEIGHT,
            ) {
                send_notice(tx, "whisper target out of range")?;
                return Ok(());
            }
            recipients.push(user.user_id.clone());
            if target_user.user_id != user.user_id {
                recipients.push(target_user.user_id.clone());
            }
        }
        ChatScope::Room => {
            if user.position.map_id == "street" {
                send_error(tx, "invalid_command", "not in a room")?;
                return Ok(());
            }
            if enc.is_none() {
                send_error(tx, "invalid_command", "room chat must be encrypted")?;
                return Ok(());
            }
            if user.position.map_id != "street" {
                recipients.extend(users_in_map.iter().map(|u| u.user_id.clone()));
            }
        }
        ChatScope::Local => {
            if user.position.map_id != "street" && enc.is_none() {
                send_error(tx, "invalid_command", "room chat must be encrypted")?;
                return Ok(());
            }
            if user.position.map_id == "street" && enc.is_some() {
                send_error(tx, "invalid_command", "street chat cannot be encrypted")?;
                return Ok(());
            }
            for other in users_in_map {
                let allow = if user.position.map_id == "street" {
                    in_box(
                        user.position.x,
                        user.position.y,
                        other.position.x,
                        other.position.y,
                        LOCAL_CHAT_WIDTH,
                        LOCAL_CHAT_HEIGHT,
                    )
                } else {
                    true
                };
                if allow {
                    recipients.push(other.user_id.clone());
                }
            }
        }
    }

    let chat = ServerChat {
        from: user.user_id.clone(),
        display_name: user.display_name.clone(),
        text,
        scope: scope.clone(),
        room_id,
        enc,
    };

    let no_recipients = recipients.is_empty();
    for user_id in &recipients {
        if let Some(handle) = state_guard.clients.get(user_id) {
            send_envelope(&handle.tx, "server.chat", &chat)?;
        }
    }

    if no_recipients {
        send_notice(tx, "no recipients")?;
    }

    Ok(())
}

async fn handle_room_key(
    payload: ClientRoomKey,
    user: &UserState,
    state: &Arc<RwLock<RelayState>>,
    tx: &mpsc::UnboundedSender<Message>,
) -> anyhow::Result<()> {
    let Some(current_room_id) = user.position.map_id.strip_prefix("room/") else {
        send_error(tx, "invalid_command", "not in a room")?;
        return Ok(());
    };
    if payload.room_id != current_room_id {
        send_error(tx, "invalid_command", "room mismatch")?;
        return Ok(());
    }
    if payload.sender_key.is_empty() || payload.ciphertext.is_empty() || payload.nonce.is_empty() {
        send_error(tx, "invalid_command", "invalid room key payload")?;
        return Ok(());
    }

    let state_guard = state.read().await;
    let users_in_map = connected_users_in_map(&state_guard, &user.position.map_id);
    let target = users_in_map
        .iter()
        .find(|u| u.user_id == payload.target)
        .map(|u| u.user_id.clone());
    let target = match target {
        Some(target) => target,
        None => {
            send_error(tx, "invalid_command", "target not found in room")?;
            return Ok(());
        }
    };

    if let Some(handle) = state_guard.clients.get(&target) {
        let env = ServerRoomKey {
            room_id: payload.room_id,
            from: user.user_id.clone(),
            sender_key: payload.sender_key,
            nonce: payload.nonce,
            ciphertext: payload.ciphertext,
        };
        send_envelope(&handle.tx, "server.room_key", &env)?;
    }

    Ok(())
}

async fn handle_command(
    payload: ClientCommand,
    user: &mut UserState,
    state: &Arc<RwLock<RelayState>>,
    storage: &Arc<Storage>,
    wallet: &MockWallet,
    config: &RelayConfig,
    tx: &mpsc::UnboundedSender<Message>,
) -> anyhow::Result<()> {
    match payload.name.as_str() {
        "who" => {
            handle_who(user, state, tx).await?;
        }
        "buy" => {
            handle_buy(user, state, storage, wallet, config, tx).await?;
        }
        "pay" => {
            handle_pay(payload.args, user, state, wallet, config, tx).await?;
        }
        "claim_name" => {
            handle_claim_name(payload.args, user, state, storage, wallet, config, tx).await?;
        }
        "access" => {
            handle_access_command(payload.args, user, state, storage, config, tx).await?;
        }
        "help" => {
            handle_help(tx)?;
        }
        "balance" => {
            handle_balance(user, wallet, tx)?;
        }
        "room_info" => {
            handle_room_info(payload.args, state, config, tx).await?;
        }
        "faucet" => {
            handle_faucet(payload.args, user, wallet, tx)?;
        }
        "board" => {
            handle_board(payload.args, user, state, tx).await?;
        }
        "depart" => {
            handle_depart(payload.args, user, state, tx).await?;
        }
        "room_name" => {
            handle_room_name(payload.args, user, state, storage, config, tx).await?;
        }
        "door_color" => {
            handle_door_color(payload.args, user, state, storage, config, tx).await?;
        }
        _ => {
            send_error(tx, "invalid_command", "unknown command")?;
        }
    }
    Ok(())
}

async fn handle_room_access(
    payload: ClientRoomAccessUpdate,
    user: &UserState,
    state: &Arc<RwLock<RelayState>>,
    storage: &Arc<Storage>,
    config: &RelayConfig,
    tx: &mpsc::UnboundedSender<Message>,
) -> anyhow::Result<()> {
    if !is_adjacent_to_customizer(user) {
        send_error(tx, "invalid_command", "must be adjacent to room customizer")?;
        return Ok(());
    }
    let mut state_guard = state.write().await;
    let room = state_guard.get_or_create_room(&payload.room_id, &config.room_price_xmr);
    if room.owner_pubkey.as_deref() != Some(&user.pubkey) {
        send_error(tx, "room_access_denied", "not room owner")?;
        return Ok(());
    }
    let updated = RoomState {
        access: AccessPolicy {
            mode: payload.mode,
            list: payload.list,
        },
        ..room
    };
    state_guard.rooms.insert(payload.room_id.clone(), updated.clone());
    let rooms_snapshot = state_guard.rooms.values().cloned().collect::<Vec<_>>();
    drop(state_guard);
    storage.save_rooms_async(rooms_snapshot).await?;
    send_room_info(tx, &updated)?;
    Ok(())
}

async fn handle_who(
    user: &UserState,
    state: &Arc<RwLock<RelayState>>,
    tx: &mpsc::UnboundedSender<Message>,
) -> anyhow::Result<()> {
    let state_guard = state.read().await;
    let mut users = Vec::new();
    let users_in_map = connected_users_in_map(&state_guard, &user.position.map_id);
    for other in users_in_map {
        if user.position.map_id == "street" {
            if !in_box(user.position.x, user.position.y, other.position.x, other.position.y, LOCAL_CHAT_WIDTH, LOCAL_CHAT_HEIGHT) {
                continue;
            }
        }
        users.push(WhoUser {
            id: other.user_id.clone(),
            display_name: other.display_name.clone(),
        });
    }
    let payload = ServerWho { users };
    send_envelope(tx, "server.who", &payload)?;
    Ok(())
}

async fn handle_buy(
    user: &UserState,
    state: &Arc<RwLock<RelayState>>,
    storage: &Arc<Storage>,
    wallet: &MockWallet,
    config: &RelayConfig,
    tx: &mpsc::UnboundedSender<Message>,
) -> anyhow::Result<()> {
    let (side, street_x) = match parse_room_map_id(&user.position.map_id) {
        Some(value) => value,
        None => {
            send_error(tx, "invalid_command", "not in a room")?;
            return Ok(());
        }
    };
    let room_id = room_id_for_door(side, street_x);
    let mut state_guard = state.write().await;
    let room = state_guard.get_or_create_room(&room_id, &config.room_price_xmr);
    if !room.for_sale {
        send_error(tx, "invalid_command", "room not for sale")?;
        return Ok(());
    }

    let price = room.price_xmr.clone();
    let fee = compute_fee(&price, &config.dev_fee)?;

    if let Some(owner_pubkey) = &room.owner_pubkey {
        if wallet_send(wallet, &user.pubkey, owner_pubkey, &price, "0", tx).is_none() {
            return Ok(());
        }
    } else if wallet_send(
        wallet,
        &user.pubkey,
        &config.dev_wallet_pubkey,
        &price,
        "0",
        tx,
    )
    .is_none()
    {
        return Ok(());
    }

    if wallet_send(
        wallet,
        &user.pubkey,
        &config.dev_wallet_pubkey,
        &fee,
        "0",
        tx,
    )
    .is_none()
    {
        return Ok(());
    }

    let mut updated = room.clone();
    updated.owner_pubkey = Some(user.pubkey.clone());
    updated.for_sale = false;
    state_guard.rooms.insert(room_id.clone(), updated.clone());
    let rooms_snapshot = state_guard.rooms.values().cloned().collect::<Vec<_>>();
    drop(state_guard);
    storage.save_rooms_async(rooms_snapshot).await?;

    let tx_update = ServerTxUpdate {
        tx_id: uuid::Uuid::new_v4().to_string(),
        status: "confirmed".to_string(),
        confirmations: 8,
    };
    send_envelope(tx, "server.tx_update", &tx_update)?;
    send_room_info(tx, &updated)?;
    handle_balance(user, wallet, tx)?;
    log_info(&format!("user {} bought room {} for {} XMR", user.user_id, room_id, price));
    Ok(())
}

async fn handle_pay(
    args: Vec<String>,
    user: &UserState,
    state: &Arc<RwLock<RelayState>>,
    wallet: &MockWallet,
    config: &RelayConfig,
    tx: &mpsc::UnboundedSender<Message>,
) -> anyhow::Result<()> {
    if args.len() < 2 {
        send_error(tx, "invalid_command", "usage: /pay <user> <amount>")?;
        return Ok(());
    }
    let target = &args[0];
    let amount = &args[1];

    let state_guard = state.read().await;
    let recipient = state_guard
        .users
        .values()
        .find(|u| u.user_id == *target || u.display_name.as_deref() == Some(target))
        .cloned();

    let recipient = match recipient {
        Some(user) => user,
        None => {
            send_error(tx, "invalid_command", "recipient not found")?;
            return Ok(());
        }
    };

    let fee = compute_fee(amount, &config.dev_fee)?;
    let tx_id = match wallet_send(wallet, &user.pubkey, &recipient.pubkey, amount, "0", tx) {
        Some(tx_id) => tx_id,
        None => return Ok(()),
    };
    if wallet_send(
        wallet,
        &user.pubkey,
        &config.dev_wallet_pubkey,
        &fee,
        "0",
        tx,
    )
    .is_none()
    {
        return Ok(());
    }

    let tx_update = ServerTxUpdate {
        tx_id,
        status: "pending".to_string(),
        confirmations: 0,
    };
    send_envelope(tx, "server.tx_update", &tx_update)?;
    handle_balance(user, wallet, tx)?;
    log_info(&format!(
        "user {} paid {} {} XMR",
        user.user_id, recipient.user_id, amount
    ));
    Ok(())
}

async fn handle_claim_name(
    args: Vec<String>,
    user: &mut UserState,
    state: &Arc<RwLock<RelayState>>,
    storage: &Arc<Storage>,
    wallet: &MockWallet,
    config: &RelayConfig,
    tx: &mpsc::UnboundedSender<Message>,
) -> anyhow::Result<()> {
    if args.is_empty() {
        send_error(tx, "invalid_command", "usage: /claim_name <name>")?;
        return Ok(());
    }
    let name = &args[0];
    let mut state_guard = state.write().await;
    if state_guard
        .users
        .values()
        .any(|u| u.display_name.as_deref() == Some(name))
    {
        send_error(tx, "invalid_command", "name already taken")?;
        return Ok(());
    }
    let fee = compute_fee(&config.username_fee_xmr, &config.dev_fee)?;
    if wallet_send(
        wallet,
        &user.pubkey,
        &config.dev_wallet_pubkey,
        &config.username_fee_xmr,
        "0",
        tx,
    )
    .is_none()
    {
        return Ok(());
    }
    if wallet_send(
        wallet,
        &user.pubkey,
        &config.dev_wallet_pubkey,
        &fee,
        "0",
        tx,
    )
    .is_none()
    {
        return Ok(());
    }

    user.display_name = Some(name.to_string());
    if let Some(entry) = state_guard.users.get_mut(&user.user_id) {
        entry.display_name = user.display_name.clone();
    }
    let users_snapshot = state_guard.users.values().cloned().collect::<Vec<_>>();
    drop(state_guard);
    storage.save_users_async(users_snapshot).await?;
    send_notice(tx, "name updated")?;
    handle_balance(user, wallet, tx)?;
    log_info(&format!("user {} claimed name {}", user.user_id, name));
    Ok(())
}

async fn handle_access_command(
    args: Vec<String>,
    user: &UserState,
    state: &Arc<RwLock<RelayState>>,
    storage: &Arc<Storage>,
    config: &RelayConfig,
    tx: &mpsc::UnboundedSender<Message>,
) -> anyhow::Result<()> {
    let (side, street_x) = match parse_room_map_id(&user.position.map_id) {
        Some(value) => value,
        None => {
            send_error(tx, "invalid_command", "not in a room")?;
            return Ok(());
        }
    };
    let room_id = room_id_for_door(side, street_x);

    if args.is_empty() || args[0].as_str() == "show" || args[0].as_str() == "list" {
        let mut state_guard = state.write().await;
        let room = state_guard.get_or_create_room(&room_id, &config.room_price_xmr);
        let list = if room.access.list.is_empty() {
            "(empty)".to_string()
        } else {
            room.access.list.join(", ")
        };
        let message = format!("access: {:?}\nlist: {}", room.access.mode, list);
        send_notice(tx, &message)?;
        return Ok(());
    }
    if !is_adjacent_to_customizer(user) {
        send_error(tx, "invalid_command", "must be adjacent to room customizer")?;
        return Ok(());
    }

    let mode = match args[0].as_str() {
        "open" => AccessMode::Open,
        "whitelist" => AccessMode::Whitelist,
        "blacklist" => AccessMode::Blacklist,
        _ => {
            send_error(tx, "invalid_command", "invalid access mode")?;
            return Ok(());
        }
    };

    let mut state_guard = state.write().await;
    let room = state_guard.get_or_create_room(&room_id, &config.room_price_xmr);
    if room.owner_pubkey.as_deref() != Some(&user.pubkey) {
        send_error(tx, "room_access_denied", "not room owner")?;
        return Ok(());
    }

    let list = match resolve_pubkeys(args.into_iter().skip(1).collect(), &state_guard) {
        Ok(list) => list,
        Err(err) => {
            send_error(tx, "invalid_command", &err.to_string())?;
            return Ok(());
        }
    };
    let updated = RoomState {
        access: AccessPolicy { mode, list },
        ..room
    };
    state_guard.rooms.insert(room_id.clone(), updated.clone());
    let rooms_snapshot = state_guard.rooms.values().cloned().collect::<Vec<_>>();
    drop(state_guard);
    storage.save_rooms_async(rooms_snapshot).await?;
    send_room_info(tx, &updated)?;
    log_info(&format!("user {} updated access {} {:?}", user.user_id, room_id, updated.access.mode));
    Ok(())
}

fn resolve_pubkeys(args: Vec<String>, state: &RelayState) -> anyhow::Result<Vec<String>> {
    let mut results = Vec::new();
    for token in args {
        if state
            .users
            .values()
            .any(|u| u.pubkey == token)
        {
            results.push(token);
            continue;
        }
        let mut matches = state
            .users
            .values()
            .filter(|u| u.user_id == token || u.display_name.as_deref() == Some(&token))
            .map(|u| u.pubkey.clone())
            .collect::<Vec<_>>();
        if matches.is_empty() {
            return Err(anyhow::anyhow!("unknown user: {token}"));
        }
        if matches.len() > 1 {
            return Err(anyhow::anyhow!("ambiguous user: {token}"));
        }
        results.append(&mut matches);
    }
    Ok(results)
}

fn handle_help(tx: &mpsc::UnboundedSender<Message>) -> anyhow::Result<()> {
    let text = [
        "commands:",
        "/say <msg> - local chat",
        "/who - list nearby users",
        "/whisper <user> <msg> - 5x5 local whisper",
        "/buy - buy current room",
        "/pay <user> <amount> - send XMR (mock)",
        "/balance - show balance (mock)",
        "/faucet [amount] - dev credit (mock)",
        "/board <north|east|south|west> - board train from station",
        "/depart <north|east|south|west> - set train destination",
        "/room_name <name> - set room name",
        "/door_color <color> - set door color",
        "/claim_name <name> - purchase unique name",
        "/access <open|whitelist|blacklist> [user|pubkey...] - set room access",
        "/access show - show current access list",
        "/exit - client quit",
        "/help - show this help",
    ]
    .join("\n");
    send_notice(tx, &text)
}

async fn handle_room_name(
    args: Vec<String>,
    user: &UserState,
    state: &Arc<RwLock<RelayState>>,
    storage: &Arc<Storage>,
    config: &RelayConfig,
    tx: &mpsc::UnboundedSender<Message>,
) -> anyhow::Result<()> {
    if !is_adjacent_to_customizer(user) {
        send_error(tx, "invalid_command", "must be adjacent to room customizer")?;
        return Ok(());
    }
    let name = args.join(" ");
    if name.is_empty() {
        send_error(tx, "invalid_command", "usage: /room_name <name>")?;
        return Ok(());
    }
    let (side, street_x) = match parse_room_map_id(&user.position.map_id) {
        Some(value) => value,
        None => {
            send_error(tx, "invalid_command", "not in a room")?;
            return Ok(());
        }
    };
    let room_id = room_id_for_door(side, street_x);
    let mut state_guard = state.write().await;
    let room = state_guard.get_or_create_room(&room_id, &config.room_price_xmr);
    if room.owner_pubkey.as_deref() != Some(&user.pubkey) {
        send_error(tx, "room_access_denied", "not room owner")?;
        return Ok(());
    }
    let updated = RoomState {
        display_name: Some(name),
        ..room
    };
    state_guard.rooms.insert(room_id.clone(), updated.clone());
    let rooms_snapshot = state_guard.rooms.values().cloned().collect::<Vec<_>>();
    drop(state_guard);
    storage.save_rooms_async(rooms_snapshot).await?;
    send_room_info(tx, &updated)?;
    Ok(())
}

async fn handle_door_color(
    args: Vec<String>,
    user: &UserState,
    state: &Arc<RwLock<RelayState>>,
    storage: &Arc<Storage>,
    config: &RelayConfig,
    tx: &mpsc::UnboundedSender<Message>,
) -> anyhow::Result<()> {
    if !is_adjacent_to_customizer(user) {
        send_error(tx, "invalid_command", "must be adjacent to room customizer")?;
        return Ok(());
    }
    let color = args.get(0).cloned().unwrap_or_default();
    if color.is_empty() {
        send_error(tx, "invalid_command", "usage: /door_color <color>")?;
        return Ok(());
    }
    let color = color.to_lowercase();
    let allowed = ["red", "green", "yellow", "blue", "magenta", "cyan", "white"];
    if !allowed.contains(&color.as_str()) {
        send_error(tx, "invalid_command", "invalid color")?;
        return Ok(());
    }
    let (side, street_x) = match parse_room_map_id(&user.position.map_id) {
        Some(value) => value,
        None => {
            send_error(tx, "invalid_command", "not in a room")?;
            return Ok(());
        }
    };
    let room_id = room_id_for_door(side, street_x);
    let mut state_guard = state.write().await;
    let room = state_guard.get_or_create_room(&room_id, &config.room_price_xmr);
    if room.owner_pubkey.as_deref() != Some(&user.pubkey) {
        send_error(tx, "room_access_denied", "not room owner")?;
        return Ok(());
    }
    let updated = RoomState {
        door_color: Some(color),
        ..room
    };
    state_guard.rooms.insert(room_id.clone(), updated.clone());
    let rooms_snapshot = state_guard.rooms.values().cloned().collect::<Vec<_>>();
    drop(state_guard);
    storage.save_rooms_async(rooms_snapshot).await?;
    send_room_info(tx, &updated)?;
    Ok(())
}

fn handle_balance(
    user: &UserState,
    wallet: &MockWallet,
    tx: &mpsc::UnboundedSender<Message>,
) -> anyhow::Result<()> {
    match wallet.get_balance(&user.pubkey) {
        Ok(balance) => send_notice(tx, &format!("balance: {} XMR", balance)),
        Err(err) => send_error(tx, "wallet_error", &err.to_string()),
    }
}

fn handle_faucet(
    args: Vec<String>,
    user: &UserState,
    wallet: &MockWallet,
    tx: &mpsc::UnboundedSender<Message>,
) -> anyhow::Result<()> {
    let amount = args
        .get(0)
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(5.0);
    wallet.credit(&user.pubkey, amount);
    send_notice(tx, &format!("faucet: +{:.8} XMR", amount))?;
    handle_balance(user, wallet, tx)
}

async fn handle_board(
    args: Vec<String>,
    user: &UserState,
    state: &Arc<RwLock<RelayState>>,
    tx: &mpsc::UnboundedSender<Message>,
) -> anyhow::Result<()> {
    let station_x = match parse_station_map_id(&user.position.map_id) {
        Some(value) => value,
        None => {
            send_error(tx, "invalid_command", "not in a station")?;
            return Ok(());
        }
    };
    let destination = match parse_station_arg(&args) {
        Some(value) => value,
        None => {
            send_error(tx, "invalid_command", "usage: /board <north|east|south|west>")?;
            return Ok(());
        }
    };
    if destination == station_x {
        send_error(tx, "invalid_command", "already at destination")?;
        return Ok(());
    }

    let mut state_guard = state.write().await;
    if state_guard.riders.contains_key(&user.user_id) {
        send_error(tx, "invalid_command", "already on train")?;
        return Ok(());
    }
    state_guard.boarding.insert(
        user.user_id.clone(),
        BoardingRequest {
            station_x,
            destination_x: destination,
        },
    );
    send_notice(tx, "waiting for train")
}

async fn handle_depart(
    args: Vec<String>,
    user: &UserState,
    state: &Arc<RwLock<RelayState>>,
    tx: &mpsc::UnboundedSender<Message>,
) -> anyhow::Result<()> {
    if parse_train_map_id(&user.position.map_id).is_none() {
        send_error(tx, "invalid_command", "not on a train")?;
        return Ok(());
    }
    let destination = match parse_station_arg(&args) {
        Some(value) => value,
        None => {
            send_error(tx, "invalid_command", "usage: /depart <north|east|south|west>")?;
            return Ok(());
        }
    };
    let mut state_guard = state.write().await;
    if let Some(ride) = state_guard.riders.get_mut(&user.user_id) {
        ride.destination_x = destination;
        send_notice(tx, "destination updated")?;
    } else {
        send_error(tx, "invalid_command", "not riding")?;
    }
    Ok(())
}

fn parse_station_arg(args: &[String]) -> Option<i64> {
    let stations = station_positions();
    let arg = args.get(0)?.as_str();
    if let Some(value) = station_x_for_label(arg) {
        return Some(value);
    }
    let value = arg.parse::<i64>().ok()?;
    if stations.iter().any(|station| *station == value) {
        Some(value)
    } else {
        None
    }
}

fn is_adjacent_to_customizer(user: &UserState) -> bool {
    let _ = match parse_room_map_id(&user.position.map_id) {
        Some(value) => value,
        None => return false,
    };
    let (cx, cy) = room_customizer_position();
    let dx = (user.position.x - cx).abs();
    let dy = (user.position.y - cy).abs();
    dx + dy == 1
}

async fn handle_room_info(
    args: Vec<String>,
    state: &Arc<RwLock<RelayState>>,
    config: &RelayConfig,
    tx: &mpsc::UnboundedSender<Message>,
) -> anyhow::Result<()> {
    if args.is_empty() {
        send_error(tx, "invalid_command", "usage: room_info <room_id>")?;
        return Ok(());
    }
    let room_id = &args[0];
    if parse_room_id(room_id).is_none() {
        send_error(tx, "invalid_command", "invalid room_id")?;
        return Ok(());
    }
    let mut state_guard = state.write().await;
    let room = state_guard.get_or_create_room(room_id, &config.room_price_xmr);
    send_room_info(tx, &room)?;
    Ok(())
}

fn compute_fee(amount: &str, config: &street_common::config::DevFeeConfig) -> anyhow::Result<String> {
    let amount_value: f64 = amount.parse().unwrap_or(0.0);
    let fee = match config.mode {
        street_common::config::DevFeeMode::Bps => amount_value * (config.value as f64) / 10000.0,
        street_common::config::DevFeeMode::Percent => amount_value * (config.value as f64) / 100.0,
    };
    Ok(format!("{:.8}", fee))
}

fn wallet_send(
    wallet: &MockWallet,
    from: &str,
    to: &str,
    amount: &str,
    fee: &str,
    tx: &mpsc::UnboundedSender<Message>,
) -> Option<String> {
    match wallet.send(from, to, amount, fee) {
        Ok(tx_id) => Some(tx_id),
        Err(err) => {
            let message = err.to_string();
            let code = if message.contains("insufficient") {
                "insufficient_funds"
            } else {
                "wallet_error"
            };
            let _ = send_error(tx, code, &message);
            None
        }
    }
}

fn room_access_allowed(room: &RoomState, pubkey: &str) -> bool {
    if room.owner_pubkey.as_deref() == Some(pubkey) {
        return true;
    }
    match room.access.mode {
        AccessMode::Open => true,
        AccessMode::Whitelist => room.access.list.iter().any(|p| p == pubkey),
        AccessMode::Blacklist => !room.access.list.iter().any(|p| p == pubkey),
    }
}

fn refresh_nearby_for_map(state: &RelayState, map_id: &str) -> anyhow::Result<()> {
    let users_in_map = connected_users_in_map(state, map_id);
    if users_in_map.is_empty() {
        return Ok(());
    }
    for user in &users_in_map {
        if let Some(handle) = state.clients.get(&user.user_id) {
            let payload = ServerNearby {
                users: collect_nearby_from_list(user, &users_in_map),
            };
            send_envelope(&handle.tx, "server.nearby", &payload)?;
        }
    }
    Ok(())
}

fn connected_users_in_map<'a>(state: &'a RelayState, map_id: &str) -> Vec<&'a UserState> {
    let Some(user_ids) = state.connected_users_by_map.get(map_id) else {
        return Vec::new();
    };
    user_ids
        .iter()
        .filter_map(|id| state.users.get(id))
        .collect()
}

fn collect_nearby_from_list(user: &UserState, users_in_map: &[&UserState]) -> Vec<NearbyUser> {
    let mut users = Vec::new();
    for other in users_in_map {
        let other = *other;
        if other.user_id == user.user_id {
            continue;
        }
        users.push(NearbyUser {
            id: other.user_id.clone(),
            display_name: other.display_name.clone(),
            x: other.position.x,
            y: other.position.y,
            x25519_pubkey: other.x25519_pubkey.clone(),
        });
    }
    users
}

fn send_room_info(tx: &mpsc::UnboundedSender<Message>, room: &RoomState) -> anyhow::Result<()> {
    let payload = ServerRoomInfo {
        room_id: room.room_id.clone(),
        owner: room.owner_pubkey.clone(),
        price_xmr: room.price_xmr.clone(),
        for_sale: room.for_sale,
        access: room.access.clone(),
        display_name: room.display_name.clone(),
        door_color: room.door_color.clone(),
    };
    send_envelope(tx, "server.room_info", &payload)
}

fn send_train_state(
    tx: &mpsc::UnboundedSender<Message>,
    trains: &[TrainState],
) -> anyhow::Result<()> {
    let payload = ServerTrainState {
        trains: trains
            .iter()
            .map(|train| TrainInfo {
                id: train.id,
                x: train.x,
                clockwise: train.clockwise,
            })
            .collect(),
    };
    send_envelope(tx, "server.train_state", &payload)
}

fn should_receive_train_state(user: &UserState) -> bool {
    user.position.map_id == "street" || parse_train_map_id(&user.position.map_id).is_some()
}

fn in_box(ax: i32, ay: i32, bx: i32, by: i32, width: i32, height: i32) -> bool {
    let half_w = width / 2;
    let half_h = height / 2;
    let min_x = ax - half_w;
    let max_x = min_x + width - 1;
    let min_y = ay - half_h;
    let max_y = min_y + height - 1;
    bx >= min_x && bx <= max_x && by >= min_y && by <= max_y
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    duration.as_millis() as i64
}

fn log_info(message: &str) {
    log_line("INFO", message);
}

fn log_warn(message: &str) {
    log_line("WARN", message);
}

fn log_error(message: &str) {
    log_line("ERROR", message);
}

fn log_line(level: &str, message: &str) {
    let ts = now_ms();
    println!("[{ts}] [{level}] {message}");
}

fn spawn_train_loop(state: Arc<RwLock<RelayState>>) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_millis(200));
        let mut last = std::time::Instant::now();
        let mut train_state_tick: u32 = 0;
        loop {
            ticker.tick().await;
            let now = std::time::Instant::now();
            let dt = now.duration_since(last).as_secs_f64();
            last = now;

            let mut state_guard = state.write().await;
            let circumference = STREET_CIRCUMFERENCE_TILES as f64;
            let mut updates = Vec::new();
            for train in &mut state_guard.trains {
                let prev = train.x;
                let direction = if train.clockwise { 1.0 } else { -1.0 };
                train.x = (train.x + train.speed * direction * dt).rem_euclid(circumference);
                updates.push((train.id, prev, train.x, train.clockwise));
            }

            handle_boarding(&mut state_guard, &updates).ok();
            handle_riders(&mut state_guard, &updates).ok();

            train_state_tick = train_state_tick.wrapping_add(1);
            if train_state_tick % TRAIN_STATE_BROADCAST_TICKS == 0 {
                let payload = ServerTrainState {
                    trains: state_guard
                        .trains
                        .iter()
                        .map(|train| TrainInfo {
                            id: train.id,
                            x: train.x,
                            clockwise: train.clockwise,
                        })
                        .collect(),
                };
                for handle in state_guard.clients.values() {
                    if let Some(user) = state_guard.users.get(&handle.user_id) {
                        if should_receive_train_state(user) {
                            let _ = send_envelope(&handle.tx, "server.train_state", &payload);
                        }
                    }
                }
            }
        }
    });
}

fn handle_boarding(
    state: &mut RelayState,
    updates: &[(u32, f64, f64, bool)],
) -> anyhow::Result<()> {
    let entries = state
        .boarding
        .iter()
        .map(|(user_id, req)| (user_id.clone(), req.clone()))
        .collect::<Vec<_>>();
    let mut to_remove = Vec::new();
    for (user_id, req) in entries {
        let station = req.station_x as f64;
        if state.riders.contains_key(&user_id) {
            to_remove.push(user_id);
            continue;
        }
        let mut boarded = false;
        let mut valid_station = true;
        if let Some(user) = state.users.get(&user_id) {
            if parse_station_map_id(&user.position.map_id) != Some(req.station_x) {
                valid_station = false;
            }
        } else {
            valid_station = false;
        }
        if !valid_station {
            to_remove.push(user_id);
            continue;
        }
        for (train_id, prev, next, clockwise) in updates {
            if station_passed(*prev, *next, station, STREET_CIRCUMFERENCE_TILES as f64, *clockwise) {
                let mut from_map = None;
                let mut to_map = None;
                let mut to_pos = None;
                let (prev_map, next_map, next_pos) = if let Some(user) = state.users.get_mut(&user_id) {
                    let previous_map = user.position.map_id.clone();
                    user.position.map_id = train_map_id(*train_id);
                    user.position.x = TRAIN_WIDTH / 2;
                    user.position.y = TRAIN_HEIGHT / 2;
                    let new_map = user.position.map_id.clone();
                    let pos = (user.position.x, user.position.y, new_map.clone());
                    state.riders.insert(
                        user_id.clone(),
                        TrainRide {
                            train_id: *train_id,
                            destination_x: req.destination_x,
                        },
                    );
                    (Some(previous_map), Some(new_map), Some(pos))
                } else {
                    (None, None, None)
                };
                if let (Some(previous_map), Some(new_map), Some(pos)) = (prev_map, next_map, next_pos) {
                    state.move_connected_user(&user_id, &previous_map, &new_map);
                    from_map = Some(previous_map);
                    to_map = Some(new_map);
                    to_pos = Some(pos);
                }
                if let (Some(from_map), Some(to_map), Some((x, y, map_id))) = (from_map, to_map, to_pos) {
                    if let Some(handle) = state.clients.get(&user_id) {
                        let payload = ServerMapChange {
                            map_id: map_id.clone(),
                            position: street_protocol::Position {
                                map_id,
                                x,
                                y,
                            },
                        };
                        let _ = send_envelope(&handle.tx, "server.map_change", &payload);
                        let _ = send_notice(&handle.tx, "boarded train");
                    }
                    let _ = refresh_nearby_for_map(state, &from_map);
                    let _ = refresh_nearby_for_map(state, &to_map);
                }
                to_remove.push(user_id.clone());
                boarded = true;
                break;
            }
        }
        if boarded {
            continue;
        }
    }
    for user_id in to_remove {
        state.boarding.remove(&user_id);
    }
    Ok(())
}

fn handle_riders(state: &mut RelayState, updates: &[(u32, f64, f64, bool)]) -> anyhow::Result<()> {
    let entries = state
        .riders
        .iter()
        .map(|(user_id, ride)| (user_id.clone(), ride.clone()))
        .collect::<Vec<_>>();
    let mut to_remove = Vec::new();
    for (user_id, ride) in entries {
        let Some((_, prev, next, clockwise)) = updates.iter().find(|(id, _, _, _)| *id == ride.train_id) else {
            continue;
        };
        let valid_train = match state.users.get(&user_id) {
            Some(user) => user.position.map_id == train_map_id(ride.train_id),
            None => false,
        };
        if !valid_train {
            to_remove.push(user_id);
            continue;
        }
        let dest = ride.destination_x as f64;
        if station_passed(*prev, *next, dest, STREET_CIRCUMFERENCE_TILES as f64, *clockwise) {
            let mut from_map = None;
            let mut to_map = None;
            let mut to_pos = None;
            let (prev_map, next_map, next_pos) = if let Some(user) = state.users.get_mut(&user_id) {
                let previous_map = user.position.map_id.clone();
                let station_map = street_world::monorail::station_map_id(ride.destination_x);
                let street_y = if *clockwise {
                    STATION_DOOR_Y_BOTTOM
                } else {
                    STATION_DOOR_Y_TOP
                };
                let (sx, sy) = street_world::station_entry_position_for_street_y(street_y);
                user.position.map_id = station_map.clone();
                user.position.x = sx;
                user.position.y = sy;
                let new_map = user.position.map_id.clone();
                let pos = (user.position.x, user.position.y, new_map.clone());
                (Some(previous_map), Some(new_map), Some(pos))
            } else {
                (None, None, None)
            };
            if let (Some(previous_map), Some(new_map), Some(pos)) = (prev_map, next_map, next_pos) {
                state.move_connected_user(&user_id, &previous_map, &new_map);
                from_map = Some(previous_map);
                to_map = Some(new_map);
                to_pos = Some(pos);
            }
            if let (Some(from_map), Some(to_map), Some((x, y, map_id))) = (from_map, to_map, to_pos) {
                if let Some(handle) = state.clients.get(&user_id) {
                    let payload = ServerMapChange {
                        map_id: map_id.clone(),
                        position: street_protocol::Position {
                            map_id,
                            x,
                            y,
                        },
                    };
                    let _ = send_envelope(&handle.tx, "server.map_change", &payload);
                    let _ = send_notice(&handle.tx, "disembarked train");
                }
                let _ = refresh_nearby_for_map(state, &from_map);
                let _ = refresh_nearby_for_map(state, &to_map);
            }
            to_remove.push(user_id);
        }
    }
    for user_id in to_remove {
        state.riders.remove(&user_id);
    }
    Ok(())
}

fn station_passed(prev: f64, next: f64, station: f64, _circumference: f64, clockwise: bool) -> bool {
    if (prev - next).abs() < f64::EPSILON {
        return false;
    }
    if clockwise {
        if prev <= next {
            station >= prev && station <= next
        } else {
            station >= prev || station <= next
        }
    } else {
        if next <= prev {
            station >= next && station <= prev
        } else {
            station >= next || station <= prev
        }
    }
}
