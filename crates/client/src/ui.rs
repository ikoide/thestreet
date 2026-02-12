use std::io::{self};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tokio::sync::mpsc;
use tokio::time::interval;
use x25519_dalek::StaticSecret;

use street_common::ids::new_message_id;
use street_protocol::signing::sign_envelope;
use street_protocol::{
    ChatScope, ClientChat, ClientCommand, ClientMove, ClientRoomKey, EncryptedPayload, Envelope,
    ServerChat, ServerError, ServerMapChange, ServerNearby, ServerNotice, ServerRoomInfo,
    ServerRoomKey, ServerState, ServerTrainState, ServerTxUpdate, ServerWelcome, ServerWho,
    TrainInfo,
};
use street_world::{
    distance_to_nearest_door, parse_room_map_id, room_customizer_position, room_id_for_door,
    street_door_side, STREET_CIRCUMFERENCE_TILES, STREET_HEIGHT,
};
use street_world::monorail::{
    is_station_door, parse_station_map_id, parse_train_map_id, station_label_for_x,
    station_positions, station_x_for_coord, station_x_for_label,
};

use crate::input::InputEvent;
use crate::render::draw_ui;
use crate::net::OutgoingMessage;
use crate::crypto::{
    decrypt_with_key, decrypt_with_shared, encrypt_with_key, encrypt_with_shared, generate_room_key,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatInputMode {
    Say,
    Whisper,
}

impl ChatInputMode {
    fn next(self) -> Self {
        match self {
            ChatInputMode::Say => ChatInputMode::Whisper,
            ChatInputMode::Whisper => ChatInputMode::Say,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub map_id: String,
    pub position: (i32, i32),
    pub nearby: Vec<street_protocol::NearbyUser>,
    pub nearby_positions: HashSet<(i32, i32)>,
    pub trains: Vec<TrainInfo>,
    pub chat_log: VecDeque<String>,
    pub chat_text: String,
    pub info_lines: Vec<String>,
    pub info_pages: Vec<Vec<String>>,
    pub info_index: usize,
    pub info_text: String,
    pub input: String,
    pub input_mode: bool,
    pub chat_mode: ChatInputMode,
    pub last_whisper_target: Option<String>,
    pub x25519_secret: [u8; 32],
    pub x25519_pubkey: String,
    pub room_keys: HashMap<String, [u8; 32]>,
    pub room_key_sent: HashMap<String, HashSet<String>>,
    pub display_name: Option<String>,
    pub user_id: String,
    pub balance: Option<String>,
    pub room_cache: HashMap<String, ServerRoomInfo>,
    pub room_info_queue: VecDeque<String>,
    pub room_info_pending: HashSet<String>,
    pub desired_destination: Option<i64>,
    pub last_view_width: i32,
    pub last_position: (i32, i32),
    pub last_map_id: String,
}

impl AppState {
    pub fn from_welcome(
        welcome: &ServerWelcome,
        x25519_secret: [u8; 32],
        x25519_pubkey: String,
    ) -> Self {
        let mut state = Self {
            map_id: welcome.position.map_id.clone(),
            position: (welcome.position.x, welcome.position.y),
            nearby: Vec::new(),
            nearby_positions: HashSet::new(),
            trains: Vec::new(),
            chat_log: VecDeque::new(),
            chat_text: String::new(),
            info_lines: Vec::new(),
            info_pages: Vec::new(),
            info_index: 0,
            info_text: String::new(),
            input: String::new(),
            input_mode: false,
            chat_mode: ChatInputMode::Say,
            last_whisper_target: None,
            x25519_secret,
            x25519_pubkey,
            room_keys: HashMap::new(),
            room_key_sent: HashMap::new(),
            display_name: welcome.display_name.clone(),
            user_id: welcome.client_id.clone(),
            balance: None,
            room_cache: HashMap::new(),
            room_info_queue: VecDeque::new(),
            room_info_pending: HashSet::new(),
            desired_destination: None,
            last_view_width: -1,
            last_position: (i32::MIN, i32::MIN),
            last_map_id: String::new(),
        }
        ;
        state.rebuild_info_pages();
        state
    }

    pub fn location_label(&self) -> String {
        if self.map_id == "street" {
            "The Street".to_string()
        } else if let Some(room_id) = self.map_id.strip_prefix("room/") {
            if let Some(room) = self.room_cache.get(room_id) {
                if let Some(name) = &room.display_name {
                    return name.clone();
                }
            }
            format!("Room {room_id}")
        } else if let Some(station_x) = parse_station_map_id(&self.map_id) {
            if let Some(label) = station_label_for_x(station_x) {
                format!("Station {label}")
            } else {
                format!("Station {station_x}")
            }
        } else if let Some(train_id) = parse_train_map_id(&self.map_id) {
            format!("Train {train_id}")
        } else {
            self.map_id.clone()
        }
    }

    pub fn push_chat(&mut self, line: String) {
        self.chat_log.push_front(line);
        while self.chat_log.len() > 200 {
            self.chat_log.pop_back();
        }
        self.refresh_chat_text();
    }

    pub fn info_title(&self) -> String {
        let total = self.info_pages.len();
        if total > 1 {
            format!("Info {}/{}", self.info_index + 1, total)
        } else {
            "Info".to_string()
        }
    }

    pub fn position_label(&self) -> String {
        let x = self.ring_x();
        let circumference = STREET_CIRCUMFERENCE_TILES as f64;
        let wrapped = (x.rem_euclid(STREET_CIRCUMFERENCE_TILES) as f64) / circumference;
        let percent = wrapped * 100.0;
        let offset = distance_to_nearest_door(x as i32);
        format!("{:>6.2}% | offset: {}m", percent, offset)
    }

    pub fn cycle_chat_mode(&mut self) {
        self.chat_mode = self.chat_mode.next();
    }

    pub fn chat_mode_label(&self) -> String {
        match self.chat_mode {
            ChatInputMode::Say => "say".to_string(),
            ChatInputMode::Whisper => match &self.last_whisper_target {
                Some(target) => format!("whisper:{target}"),
                None => "whisper:none".to_string(),
            },
        }
    }

    pub fn input_title(&self) -> String {
        format!("Input [{}]", self.chat_mode_label())
    }

    pub fn input_hint(&self) -> String {
        "Enter to type | / commands | Tab say/whisper".to_string()
    }

    pub fn rebuild_info_pages(&mut self) {
        let mut pages = Vec::new();
        let name = self.display_name.clone().unwrap_or_else(|| self.user_id.clone());
        let balance = self
            .balance
            .clone()
            .map(|b| format!("{b} XMR"))
            .unwrap_or_else(|| "(unknown)".to_string());
        let mut user_page = Vec::new();
        user_page.push(format!("user: {name}"));
        user_page.push(format!("id: {}", self.user_id));
        user_page.push(format!("balance: {balance}"));
        pages.push(user_page);

        if let Some(room_id) = self.door_adjacent_room_id() {
            let (door_page, needs_request) = self.room_page("door", &room_id);
            pages.push(door_page);
            if needs_request {
                self.request_room_info(&room_id);
            }
        }

        if let Some(station_x) = self.door_adjacent_station() {
            let label = station_label_for_x(station_x).unwrap_or("unknown");
            pages.push(vec![
                "monorail".to_string(),
                "enter: step on M".to_string(),
                format!("station: {label}"),
            ]);
        }

        if let Some(room_id) = self.map_id.strip_prefix("room/") {
            let room_id = room_id.to_string();
            let (room_page, needs_request) = self.room_page("room", &room_id);
            pages.push(room_page);
            if needs_request {
                self.request_room_info(&room_id);
            }
            if let Some(exit_page) = self.room_exit_page() {
                pages.push(exit_page);
            }
            if self.room_customizer_adjacent() {
                pages.push(room_settings_page());
            }
        }

        if let Some(station_x) = parse_station_map_id(&self.map_id) {
            pages.push(station_page(station_x));
        }

        if let Some(train_id) = parse_train_map_id(&self.map_id) {
            pages.push(train_page(train_id, self.desired_destination));
        }

        if pages.is_empty() {
            pages.push(vec!["Welcome to The Street".to_string()]);
        }

        self.info_pages = pages;
        if self.info_index >= self.info_pages.len() {
            self.info_index = 0;
        }
        self.info_lines = self.info_pages[self.info_index].clone();
        self.refresh_info_text();
    }

    pub fn cycle_info(&mut self) {
        if self.info_pages.len() <= 1 {
            return;
        }
        self.info_index = (self.info_index + 1) % self.info_pages.len();
        self.info_lines = self.info_pages[self.info_index].clone();
        self.refresh_info_text();
    }

    fn refresh_info_text(&mut self) {
        self.info_text = self.info_lines.join("\n");
    }

    fn refresh_chat_text(&mut self) {
        self.chat_text = self.chat_log.iter().cloned().collect::<Vec<_>>().join("\n");
    }

    pub fn door_adjacent_room_id(&self) -> Option<String> {
        if self.map_id != "street" {
            return None;
        }
        let (x, y) = self.position;
        if y == 1 {
            if let Some(side) = street_door_side(x, 0) {
                return Some(room_id_for_door(side, x));
            }
        }
        if y == STREET_HEIGHT - 2 {
            if let Some(side) = street_door_side(x, STREET_HEIGHT - 1) {
                return Some(room_id_for_door(side, x));
            }
        }
        None
    }

    fn door_adjacent_station(&self) -> Option<i64> {
        if self.map_id != "street" {
            return None;
        }
        let (x, y) = self.position;
        let above = is_station_door(x, y - 1);
        let below = is_station_door(x, y + 1);
        if above || below {
            return station_x_for_coord(x);
        }
        None
    }

    fn room_page(&self, label: &str, room_id: &str) -> (Vec<String>, bool) {
        if let Some(room) = self.room_cache.get(room_id) {
            let mut page = Vec::new();
            page.push(format!("{label}: {}", room.room_id));
            if let Some(name) = &room.display_name {
                page.push(format!("name: {name}"));
            }
            page.push(format!(
                "owner: {}",
                room.owner.clone().unwrap_or_else(|| "none".to_string())
            ));
            page.push(format!("price: {} XMR", room.price_xmr));
            page.push(format!("for sale: {}", if room.for_sale { "yes" } else { "no" }));
            page.push(format!("access: {:?}", room.access.mode));
            if let Some(color) = &room.door_color {
                page.push(format!("door: {color}"));
            }
            (page, false)
        } else {
            let page = vec![
                format!("{label}: {room_id}"),
                "owner: (unknown)".to_string(),
                "access: (unknown)".to_string(),
                "price: (unknown)".to_string(),
                "for sale: (unknown)".to_string(),
            ];
            (page, true)
        }
    }

    fn room_exit_page(&self) -> Option<Vec<String>> {
        let (side, _) = parse_room_map_id(&self.map_id)?;
        let (door_x, door_y) = street_world::room_door_position(side);
        let (x, y) = self.position;
        let adjacent = x == door_x && (y == door_y - 1 || y == door_y + 1);
        if !adjacent {
            return None;
        }
        Some(vec![
            "exit: The Street".to_string(),
            "step through the door".to_string(),
        ])
    }

    fn room_customizer_adjacent(&self) -> bool {
        if parse_room_map_id(&self.map_id).is_none() {
            return false;
        }
        let (cx, cy) = room_customizer_position();
        let (x, y) = self.position;
        (x - cx).abs() + (y - cy).abs() == 1
    }

    fn request_room_info(&mut self, room_id: &str) {
        self.queue_room_info_request(room_id);
    }

    fn queue_room_info_request(&mut self, room_id: &str) {
        if self.room_cache.contains_key(room_id) {
            return;
        }
        if self.room_info_pending.contains(room_id) {
            return;
        }
        self.room_info_pending.insert(room_id.to_string());
        self.room_info_queue.push_back(room_id.to_string());
    }

    fn queue_visible_room_info(&mut self, view_width: i32) {
        if self.map_id != "street" {
            return;
        }
        if view_width <= 0 {
            return;
        }
        let width = view_width.max(1);
        let player_x = self.position.0;
        let start_x = player_x - width / 2;
        for dx in 0..width {
            let x = start_x + dx;
            if let Some(side) = street_door_side(x, 0) {
                let room_id = room_id_for_door(side, x);
                self.queue_room_info_request(&room_id);
            }
            if let Some(side) = street_door_side(x, STREET_HEIGHT - 1) {
                let room_id = room_id_for_door(side, x);
                self.queue_room_info_request(&room_id);
            }
        }
    }

    pub fn ring_x(&self) -> i64 {
        if self.map_id == "street" {
            self.position.0 as i64
        } else if let Some((_, street_x)) = parse_room_map_id(&self.map_id) {
            street_x as i64
        } else if let Some(station_x) = parse_station_map_id(&self.map_id) {
            station_x
        } else if let Some(train_id) = parse_train_map_id(&self.map_id) {
            if let Some(train) = self.trains.iter().find(|t| t.id == train_id) {
                train.x.round() as i64
            } else {
                0
            }
        } else {
            0
        }
    }

    pub fn maybe_queue_visible_room_info(&mut self, view_width: i32) {
        let width = view_width.max(0);
        let needs_refresh = self.last_view_width != width
            || self.last_position != self.position
            || self.last_map_id != self.map_id;
        if !needs_refresh {
            return;
        }
        self.last_view_width = width;
        self.last_position = self.position;
        self.last_map_id = self.map_id.clone();
        self.queue_visible_room_info(width);
    }
}

fn station_page(station_x: i64) -> Vec<String> {
    let label = station_label_for_x(station_x).unwrap_or("unknown");
    let stations = station_positions();
    let mut destination_lines = Vec::new();
    for (index, station) in stations.iter().enumerate() {
        let dest_label = station_label_for_x(*station).unwrap_or("unknown");
        destination_lines.push(format!("{}) {dest_label}", index + 1));
    }
    let mut lines = vec![
        format!("station: {label}"),
        "destinations:".to_string(),
    ];
    lines.append(&mut destination_lines);
    lines.push("press 1-4 or /board <north|east|south|west>".to_string());
    lines
}

fn train_page(train_id: u32, destination: Option<i64>) -> Vec<String> {
    let dest_label = destination.and_then(station_label_for_x).map(|value| value.to_string());
    let dest_label = dest_label.or_else(|| destination.map(|value| value.to_string()));
    let dest_line = match dest_label {
        Some(label) => format!("destination: {label}"),
        None => "destination: (unset)".to_string(),
    };
    vec![
        format!("train: {train_id}"),
        dest_line,
        "press 1-4 or /depart <north|east|south|west>".to_string(),
    ]
}

fn room_settings_page() -> Vec<String> {
    vec![
        "room settings".to_string(),
        "stand next to C".to_string(),
        "use /room_name <name>".to_string(),
        "use /door_color <color>".to_string(),
        "use /access <open|whitelist|blacklist>".to_string(),
        "(owner only)".to_string(),
    ]
}

pub async fn run_ui(
    mut app: AppState,
    mut incoming: mpsc::UnboundedReceiver<Envelope>,
    outgoing: mpsc::UnboundedSender<OutgoingMessage>,
    signing_key: ed25519_dalek::SigningKey,
    mut input_rx: mpsc::UnboundedReceiver<InputEvent>,
) -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let signing_key = Arc::new(signing_key);
    let mut ticker = interval(Duration::from_millis(100));
    let mut needs_draw = true;
    let mut last_size = terminal.size()?;

    send_balance_request(&outgoing, &signing_key)?;

    loop {
        if needs_draw {
            let size = terminal.size()?;
            last_size = size;
            let view_width = size.width.saturating_sub(2) as i32;
            app.maybe_queue_visible_room_info(view_width);
            terminal.draw(|f| draw_ui(f, &app))?;
            needs_draw = false;
        }

        tokio::select! {
            _ = ticker.tick() => {
                let size = terminal.size()?;
                if size != last_size {
                    last_size = size;
                    needs_draw = true;
                }
                while let Some(room_id) = app.room_info_queue.pop_front() {
                    if app.room_cache.contains_key(&room_id) {
                        app.room_info_pending.remove(&room_id);
                        continue;
                    }
                    let _ = send_room_info_request(&outgoing, &signing_key, &room_id);
                    break;
                }
            }
            Some(env) = incoming.recv() => {
                handle_server_message(&mut app, env, &outgoing, &signing_key)?;
                needs_draw = true;
            }
            Some(input) = input_rx.recv() => {
                if handle_input_event(&mut app, input, &outgoing, &signing_key)? {
                    break;
                }
                needs_draw = true;
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    Ok(())
}

fn handle_server_message(
    app: &mut AppState,
    env: Envelope,
    outgoing: &mpsc::UnboundedSender<OutgoingMessage>,
    signing_key: &Arc<ed25519_dalek::SigningKey>,
) -> anyhow::Result<()> {
    match env.message_type.as_str() {
        "server.state" => {
            if let Ok(payload) = serde_json::from_value::<ServerState>(env.payload) {
                app.map_id = payload.position.map_id;
                app.position = (payload.position.x, payload.position.y);
                app.rebuild_info_pages();
            }
        }
        "server.map_change" => {
            if let Ok(payload) = serde_json::from_value::<ServerMapChange>(env.payload) {
                app.map_id = payload.map_id;
                app.position = (payload.position.x, payload.position.y);
                app.info_lines.clear();
                app.nearby.clear();
                app.nearby_positions.clear();
                app.room_info_queue.clear();
                app.room_info_pending.clear();
                if parse_station_map_id(&app.map_id).is_some() {
                    app.desired_destination = None;
                }
                app.rebuild_info_pages();
            }
        }
        "server.chat" => {
            if let Ok(payload) = serde_json::from_value::<ServerChat>(env.payload) {
                handle_chat_message(app, payload)?;
            }
        }
        "server.nearby" => {
            if let Ok(payload) = serde_json::from_value::<ServerNearby>(env.payload) {
                app.nearby = payload.users;
                app.nearby_positions.clear();
                app.nearby_positions
                    .extend(app.nearby.iter().map(|user| (user.x, user.y)));
                maybe_distribute_room_key(app, outgoing, signing_key)?;
            }
        }
        "server.room_key" => {
            if let Ok(payload) = serde_json::from_value::<ServerRoomKey>(env.payload) {
                handle_room_key_message(app, payload)?;
            }
        }
        "server.train_state" => {
            if let Ok(payload) = serde_json::from_value::<ServerTrainState>(env.payload) {
                app.trains = payload.trains;
            }
        }
        "server.who" => {
            if let Ok(payload) = serde_json::from_value::<ServerWho>(env.payload) {
                let names: Vec<String> = payload
                    .users
                    .into_iter()
                    .map(|u| u.display_name.unwrap_or(u.id))
                    .collect();
                app.push_chat(format!("who: {}", names.join(", ")));
            }
        }
        "server.room_info" => {
            if let Ok(payload) = serde_json::from_value::<ServerRoomInfo>(env.payload) {
                let room_id = payload.room_id.clone();
                app.room_cache.insert(room_id.clone(), payload);
                app.room_info_pending.remove(&room_id);
                app.rebuild_info_pages();
            }
        }
        "server.tx_update" => {
            if let Ok(payload) = serde_json::from_value::<ServerTxUpdate>(env.payload) {
                app.push_chat(format!(
                    "tx {}: {} ({} conf)",
                    payload.tx_id, payload.status, payload.confirmations
                ));
            }
        }
        "server.error" => {
            if let Ok(payload) = serde_json::from_value::<ServerError>(env.payload) {
                app.push_chat(format!("error: {}", payload.message));
            }
        }
        "server.notice" => {
            if let Ok(payload) = serde_json::from_value::<ServerNotice>(env.payload) {
                if let Some(balance) = payload.text.strip_prefix("balance: ") {
                    app.balance = Some(balance.trim().replace(" XMR", ""));
                    app.rebuild_info_pages();
                } else {
                    app.push_chat(payload.text);
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn handle_input_event(
    app: &mut AppState,
    input: InputEvent,
    outgoing: &mpsc::UnboundedSender<OutgoingMessage>,
    signing_key: &Arc<ed25519_dalek::SigningKey>,
) -> anyhow::Result<bool> {
    match input {
        InputEvent::Key(key) => {
            if app.input_mode {
                match key.code {
                    KeyCode::Tab if key.kind == KeyEventKind::Press => {
                        app.cycle_chat_mode();
                    }
                    KeyCode::Esc if key.kind == KeyEventKind::Press => {
                        app.input.clear();
                        app.input_mode = false;
                    }
                    KeyCode::Enter if key.kind == KeyEventKind::Press => {
                        let input = app.input.trim().to_string();
                        app.input.clear();
                        app.input_mode = false;
                        if input.is_empty() {
                            return Ok(false);
                        }
                        if handle_text_input(app, &input, outgoing, signing_key)? {
                            return Ok(true);
                        }
                    }
                    KeyCode::Backspace if key.kind == KeyEventKind::Press => {
                        app.input.pop();
                    }
                    KeyCode::Char(c) if key.kind == KeyEventKind::Press => {
                        app.input.push(c);
                    }
                    _ => {}
                }
                return Ok(false);
            }

            match key.code {
                KeyCode::Char('q') if key.modifiers.contains(event::KeyModifiers::CONTROL) && key.kind == KeyEventKind::Press => {
                    return Ok(true);
                }
                KeyCode::Esc if key.kind == KeyEventKind::Press => return Ok(true),
                KeyCode::Tab if key.kind == KeyEventKind::Press => {
                    app.cycle_chat_mode();
                }
                KeyCode::Char('i') if key.kind == KeyEventKind::Press => {
                    app.cycle_info();
                }
                KeyCode::Char('1') if key.kind == KeyEventKind::Press => {
                    let _ = handle_quick_station_action(app, outgoing, signing_key, 1)?;
                }
                KeyCode::Char('2') if key.kind == KeyEventKind::Press => {
                    let _ = handle_quick_station_action(app, outgoing, signing_key, 2)?;
                }
                KeyCode::Char('3') if key.kind == KeyEventKind::Press => {
                    let _ = handle_quick_station_action(app, outgoing, signing_key, 3)?;
                }
                KeyCode::Char('4') if key.kind == KeyEventKind::Press => {
                    let _ = handle_quick_station_action(app, outgoing, signing_key, 4)?;
                }
                KeyCode::Char('/') if key.kind == KeyEventKind::Press => {
                    app.input_mode = true;
                    app.input.clear();
                    app.input.push('/');
                }
                KeyCode::Enter if key.kind == KeyEventKind::Press => {
                    app.input_mode = true;
                    app.input.clear();
                }
                _ => {
                    if let Some(dir) = direction_from_key(&key.code) {
                        match key.kind {
                            KeyEventKind::Press | KeyEventKind::Repeat => {
                                send_move(outgoing, signing_key, dir)?;
                            }
                            KeyEventKind::Release => {}
                        }
                    }
                }
            }
        }
        InputEvent::Paste(text) => {
            if app.input_mode {
                app.input.push_str(&text);
            }
        }
    }
    Ok(false)
}

fn handle_command_input(
    app: &mut AppState,
    input: &str,
    outgoing: &mpsc::UnboundedSender<OutgoingMessage>,
    signing_key: &Arc<ed25519_dalek::SigningKey>,
) -> anyhow::Result<bool> {
    let parts: Vec<String> = input.trim_start_matches('/').split_whitespace().map(|s| s.to_string()).collect();
    if parts.is_empty() {
        return Ok(false);
    }
    let name = parts[0].clone();
    let args = parts[1..].to_vec();
    let requires_customizer = matches!(name.as_str(), "room_name" | "door_color" | "access");
    if requires_customizer && !app.room_customizer_adjacent() {
        app.push_chat("error: must be adjacent to room settings C".to_string());
        return Ok(false);
    }
    if name == "whisper" {
        if args.len() < 2 {
            app.push_chat("error: usage: /whisper <user> <msg>".to_string());
            return Ok(false);
        }
        let target = args[0].clone();
        let text = args[1..].join(" ");
        if text.is_empty() {
            app.push_chat("error: usage: /whisper <user> <msg>".to_string());
            return Ok(false);
        }
        let target_user = match resolve_target_user(app, &target) {
            Ok(user) => user,
            Err(err) => {
                app.push_chat(format!("error: {err}"));
                return Ok(false);
            }
        };
        app.last_whisper_target = Some(target_user.id.clone());
        send_whisper_chat(app, outgoing, signing_key, &target_user, &text)?;
        return Ok(false);
    }
    if name == "say" {
        let text = args.join(" ");
        if text.is_empty() {
            return Ok(false);
        }
        send_local_chat(app, outgoing, signing_key, &text)?;
        return Ok(false);
    }
    if name == "exit" || name == "quit" {
        let _ = outgoing.send(OutgoingMessage::Close);
        return Ok(true);
    }
    if name == "board" || name == "depart" {
        app.desired_destination = parse_destination_arg(&args);
        let payload = ClientCommand { name, args };
        send_signed(outgoing, signing_key, "client.command", &payload)?;
        return Ok(false);
    }
    if name == "room_name" || name == "door_color" {
        let payload = ClientCommand { name, args };
        send_signed(outgoing, signing_key, "client.command", &payload)?;
        return Ok(false);
    }
    if name == "help" {
        let payload = ClientCommand { name, args };
        send_signed(outgoing, signing_key, "client.command", &payload)?;
        return Ok(false);
    }
    let payload = ClientCommand { name, args };
    send_signed(outgoing, signing_key, "client.command", &payload)?;
    Ok(false)
}

fn handle_text_input(
    app: &mut AppState,
    input: &str,
    outgoing: &mpsc::UnboundedSender<OutgoingMessage>,
    signing_key: &Arc<ed25519_dalek::SigningKey>,
) -> anyhow::Result<bool> {
    if input.starts_with('/') {
        return handle_command_input(app, input, outgoing, signing_key);
    }
    match app.chat_mode {
        ChatInputMode::Say => {
            send_local_chat(app, outgoing, signing_key, input)?;
        }
        ChatInputMode::Whisper => {
            let Some(target) = app.last_whisper_target.clone() else {
                app.push_chat("error: no whisper target (use /whisper <user> <msg>)".to_string());
                return Ok(false);
            };
            let target_user = match find_user_by_id(app, &target) {
                Some(user) => user,
                None => {
                    app.push_chat("error: whisper target not nearby".to_string());
                    return Ok(false);
                }
            };
            send_whisper_chat(app, outgoing, signing_key, &target_user, input)?;
        }
    }
    Ok(false)
}

fn send_move(
    outgoing: &mpsc::UnboundedSender<OutgoingMessage>,
    signing_key: &Arc<ed25519_dalek::SigningKey>,
    dir: street_protocol::Direction,
) -> anyhow::Result<()> {
    let payload = ClientMove { dir };
    send_signed(outgoing, signing_key, "client.move", &payload)
}

fn send_local_chat(
    app: &mut AppState,
    outgoing: &mpsc::UnboundedSender<OutgoingMessage>,
    signing_key: &Arc<ed25519_dalek::SigningKey>,
    text: &str,
) -> anyhow::Result<()> {
    if text.trim().is_empty() {
        return Ok(());
    }
    let mut payload = ClientChat {
        scope: Some(ChatScope::Local),
        text: text.to_string(),
        target: None,
        enc: None,
    };

    if let Some(room_id) = current_room_id(app) {
        let Some(room_key) = ensure_room_key(app, outgoing, signing_key, &room_id)? else {
            app.push_chat("error: room key unavailable".to_string());
            return Ok(());
        };
        let (nonce, ciphertext) = encrypt_with_key(&room_key, text.as_bytes())?;
        payload.text.clear();
        payload.enc = Some(EncryptedPayload {
            alg: "xchacha20poly1305".to_string(),
            nonce,
            ciphertext,
            sender_key: None,
        });
    }

    send_signed(outgoing, signing_key, "client.chat", &payload)
}

fn send_whisper_chat(
    app: &mut AppState,
    outgoing: &mpsc::UnboundedSender<OutgoingMessage>,
    signing_key: &Arc<ed25519_dalek::SigningKey>,
    target: &street_protocol::NearbyUser,
    text: &str,
) -> anyhow::Result<()> {
    if text.trim().is_empty() {
        return Ok(());
    }
    let Some(peer_key) = target.x25519_pubkey.as_deref() else {
        app.push_chat("error: target missing x25519 key".to_string());
        return Ok(());
    };
    let secret = StaticSecret::from(app.x25519_secret);
    let context = whisper_context(&app.user_id, &target.id);
    let (nonce, ciphertext) = encrypt_with_shared(&secret, peer_key, context.as_bytes(), text.as_bytes())?;
    let payload = ClientChat {
        scope: Some(ChatScope::Whisper),
        text: String::new(),
        target: Some(target.id.clone()),
        enc: Some(EncryptedPayload {
            alg: "x25519-xchacha20poly1305".to_string(),
            nonce,
            ciphertext,
            sender_key: Some(app.x25519_pubkey.clone()),
        }),
    };
    send_signed(outgoing, signing_key, "client.chat", &payload)
}

fn send_signed<T: serde::Serialize>(
    outgoing: &mpsc::UnboundedSender<OutgoingMessage>,
    signing_key: &Arc<ed25519_dalek::SigningKey>,
    message_type: &str,
    payload: &T,
) -> anyhow::Result<()> {
    let env = sign_envelope(signing_key, message_type, &new_message_id(), now_ms(), payload)?;
    outgoing.send(OutgoingMessage::Envelope(env))?;
    Ok(())
}

fn send_balance_request(
    outgoing: &mpsc::UnboundedSender<OutgoingMessage>,
    signing_key: &Arc<ed25519_dalek::SigningKey>,
) -> anyhow::Result<()> {
    let payload = ClientCommand {
        name: "balance".to_string(),
        args: Vec::new(),
    };
    send_signed(outgoing, signing_key, "client.command", &payload)
}

fn send_room_info_request(
    outgoing: &mpsc::UnboundedSender<OutgoingMessage>,
    signing_key: &Arc<ed25519_dalek::SigningKey>,
    room_id: &str,
) -> anyhow::Result<()> {
    let payload = ClientCommand {
        name: "room_info".to_string(),
        args: vec![room_id.to_string()],
    };
    send_signed(outgoing, signing_key, "client.command", &payload)
}

fn current_room_id(app: &AppState) -> Option<String> {
    app.map_id.strip_prefix("room/").map(|value| value.to_string())
}

fn whisper_context(a: &str, b: &str) -> String {
    let (min_id, max_id) = if a <= b { (a, b) } else { (b, a) };
    format!("whisper:{min_id}:{max_id}")
}

fn room_key_context(room_id: &str, a: &str, b: &str) -> String {
    let (min_id, max_id) = if a <= b { (a, b) } else { (b, a) };
    format!("room-key:{room_id}:{min_id}:{max_id}")
}

fn resolve_target_user(app: &AppState, token: &str) -> anyhow::Result<street_protocol::NearbyUser> {
    let mut matches = app
        .nearby
        .iter()
        .filter(|u| u.id == token || u.display_name.as_deref() == Some(token))
        .cloned()
        .collect::<Vec<_>>();
    if matches.is_empty() {
        anyhow::bail!("whisper target not found")
    }
    if matches.len() > 1 {
        anyhow::bail!("ambiguous whisper target")
    }
    Ok(matches.remove(0))
}

fn find_user_by_id(app: &AppState, user_id: &str) -> Option<street_protocol::NearbyUser> {
    app.nearby.iter().find(|u| u.id == user_id).cloned()
}

fn room_leader_id(app: &AppState) -> String {
    let mut ids = app.nearby.iter().map(|u| u.id.clone()).collect::<Vec<_>>();
    ids.push(app.user_id.clone());
    ids.sort();
    ids.first().cloned().unwrap_or_else(|| app.user_id.clone())
}

fn ensure_room_key(
    app: &mut AppState,
    outgoing: &mpsc::UnboundedSender<OutgoingMessage>,
    signing_key: &Arc<ed25519_dalek::SigningKey>,
    room_id: &str,
) -> anyhow::Result<Option<[u8; 32]>> {
    if let Some(key) = app.room_keys.get(room_id) {
        return Ok(Some(*key));
    }
    let leader = room_leader_id(app);
    if leader != app.user_id {
        return Ok(None);
    }
    let key = generate_room_key();
    app.room_keys.insert(room_id.to_string(), key);
    app.room_key_sent.insert(room_id.to_string(), HashSet::new());
    distribute_room_key(app, outgoing, signing_key, room_id, key)?;
    Ok(Some(key))
}

fn distribute_room_key(
    app: &mut AppState,
    outgoing: &mpsc::UnboundedSender<OutgoingMessage>,
    signing_key: &Arc<ed25519_dalek::SigningKey>,
    room_id: &str,
    key: [u8; 32],
) -> anyhow::Result<()> {
    let sent = app
        .room_key_sent
        .entry(room_id.to_string())
        .or_insert_with(HashSet::new);
    for user in &app.nearby {
        if user.id == app.user_id {
            continue;
        }
        if sent.contains(&user.id) {
            continue;
        }
        let Some(peer_key) = user.x25519_pubkey.as_deref() else {
            continue;
        };
        let secret = StaticSecret::from(app.x25519_secret);
        let context = room_key_context(room_id, &app.user_id, &user.id);
        let (nonce, ciphertext) = encrypt_with_shared(&secret, peer_key, context.as_bytes(), &key)?;
        let payload = ClientRoomKey {
            room_id: room_id.to_string(),
            target: user.id.clone(),
            sender_key: app.x25519_pubkey.clone(),
            nonce,
            ciphertext,
        };
        send_signed(outgoing, signing_key, "client.room_key", &payload)?;
        sent.insert(user.id.clone());
    }
    Ok(())
}

fn maybe_distribute_room_key(
    app: &mut AppState,
    outgoing: &mpsc::UnboundedSender<OutgoingMessage>,
    signing_key: &Arc<ed25519_dalek::SigningKey>,
) -> anyhow::Result<()> {
    let Some(room_id) = current_room_id(app) else {
        return Ok(());
    };
    let key = match ensure_room_key(app, outgoing, signing_key, &room_id)? {
        Some(key) => key,
        None => return Ok(()),
    };
    let leader = room_leader_id(app);
    if leader != app.user_id {
        return Ok(());
    }
    distribute_room_key(app, outgoing, signing_key, &room_id, key)
}

fn handle_room_key_message(app: &mut AppState, payload: ServerRoomKey) -> anyhow::Result<()> {
    let secret = StaticSecret::from(app.x25519_secret);
    let context = room_key_context(&payload.room_id, &app.user_id, &payload.from);
    let plaintext = decrypt_with_shared(
        &secret,
        &payload.sender_key,
        context.as_bytes(),
        &payload.nonce,
        &payload.ciphertext,
    )?;
    let key_bytes: [u8; 32] = plaintext
        .try_into()
        .map_err(|_| anyhow::anyhow!("invalid room key"))?;
    app.room_keys.insert(payload.room_id.clone(), key_bytes);
    app.push_chat("room key received".to_string());
    Ok(())
}

fn handle_chat_message(app: &mut AppState, payload: ServerChat) -> anyhow::Result<()> {
    let name = payload.display_name.clone().unwrap_or(payload.from.clone());
    let text = if let Some(enc) = payload.enc {
        let decrypted = match payload.scope {
            ChatScope::Whisper => {
                let Some(sender_key) = enc.sender_key.as_ref() else {
                    return Ok(app.push_chat(format!("[whisper] {name}: [decrypt failed]")));
                };
                let secret = StaticSecret::from(app.x25519_secret);
                let context = whisper_context(&app.user_id, &payload.from);
                decrypt_with_shared(
                    &secret,
                    sender_key,
                    context.as_bytes(),
                    &enc.nonce,
                    &enc.ciphertext,
                )
            }
            ChatScope::Room | ChatScope::Local => {
                let room_id = payload
                    .room_id
                    .clone()
                    .or_else(|| current_room_id(app));
                let Some(room_id) = room_id else {
                    return Ok(app.push_chat(format!("{name}: [decrypt failed]")));
                };
                let Some(room_key) = app.room_keys.get(&room_id) else {
                    return Ok(app.push_chat(format!("{name}: [decrypt failed]")));
                };
                decrypt_with_key(room_key, &enc.nonce, &enc.ciphertext)
            }
        };
        match decrypted {
            Ok(bytes) => String::from_utf8(bytes).unwrap_or_else(|_| "[invalid utf8]".to_string()),
            Err(_) => "[decrypt failed]".to_string(),
        }
    } else {
        payload.text.clone()
    };

    let line = match payload.scope {
        ChatScope::Whisper => format!("[whisper] {name}: {text}"),
        _ => {
            let location = app.location_label();
            format!("[{location}] {name}: {text}")
        }
    };
    app.push_chat(line);
    Ok(())
}

fn handle_quick_station_action(
    app: &mut AppState,
    outgoing: &mpsc::UnboundedSender<OutgoingMessage>,
    signing_key: &Arc<ed25519_dalek::SigningKey>,
    choice: usize,
) -> anyhow::Result<bool> {
    let stations = station_positions();
    let destination = choice.checked_sub(1).and_then(|index| stations.get(index)).copied();
    let Some(destination) = destination else {
        return Ok(false);
    };
    if parse_station_map_id(&app.map_id).is_some() {
        app.desired_destination = Some(destination);
        let label = station_label_for_x(destination).unwrap_or("unknown");
        let payload = ClientCommand {
            name: "board".to_string(),
            args: vec![label.to_string()],
        };
        send_signed(outgoing, signing_key, "client.command", &payload)?;
        return Ok(true);
    }
    if parse_train_map_id(&app.map_id).is_some() {
        app.desired_destination = Some(destination);
        let label = station_label_for_x(destination).unwrap_or("unknown");
        let payload = ClientCommand {
            name: "depart".to_string(),
            args: vec![label.to_string()],
        };
        send_signed(outgoing, signing_key, "client.command", &payload)?;
        return Ok(true);
    }
    Ok(false)
}

fn parse_destination_arg(args: &[String]) -> Option<i64> {
    let arg = args.get(0)?.as_str();
    if let Some(value) = station_x_for_label(arg) {
        return Some(value);
    }
    arg.parse::<i64>().ok()
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    duration.as_millis() as i64
}

fn direction_from_key(code: &KeyCode) -> Option<street_protocol::Direction> {
    match code {
        KeyCode::Up | KeyCode::Char('w') => Some(street_protocol::Direction::Up),
        KeyCode::Down | KeyCode::Char('s') => Some(street_protocol::Direction::Down),
        KeyCode::Left | KeyCode::Char('a') => Some(street_protocol::Direction::Left),
        KeyCode::Right | KeyCode::Char('d') => Some(street_protocol::Direction::Right),
        _ => None,
    }
}
