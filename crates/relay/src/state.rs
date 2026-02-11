use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use street_protocol::{AccessMode, AccessPolicy};
use street_world::Position;
use street_world::STREET_CIRCUMFERENCE_TILES;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserState {
    pub user_id: String,
    pub pubkey: String,
    pub display_name: Option<String>,
    pub position: Position,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomState {
    pub room_id: String,
    pub owner_pubkey: Option<String>,
    pub price_xmr: String,
    pub for_sale: bool,
    pub access: AccessPolicy,
    pub display_name: Option<String>,
    pub door_color: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ClientHandle {
    pub user_id: String,
    pub pubkey: String,
    pub tx: mpsc::UnboundedSender<tokio_tungstenite::tungstenite::Message>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainState {
    pub id: u32,
    pub x: f64,
    pub speed: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardingRequest {
    pub station_x: i64,
    pub destination_x: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainRide {
    pub train_id: u32,
    pub destination_x: i64,
}

#[derive(Debug, Default)]
pub struct ServerState {
    pub next_user_id: u64,
    pub users: HashMap<String, UserState>,
    pub users_by_pubkey: HashMap<String, String>,
    pub rooms: HashMap<String, RoomState>,
    pub clients: HashMap<String, ClientHandle>,
    pub connected_users_by_map: HashMap<String, HashSet<String>>,
    pub trains: Vec<TrainState>,
    pub boarding: HashMap<String, BoardingRequest>,
    pub riders: HashMap<String, TrainRide>,
    pub last_move_ms: HashMap<String, i64>,
}

impl ServerState {
    pub fn new(users: Vec<UserState>, rooms: Vec<RoomState>) -> Self {
        let mut state = ServerState::default();
        for user in users {
            state.next_user_id = state
                .next_user_id
                .max(user.user_id.trim_start_matches("u_").parse().unwrap_or(0));
            state
                .users_by_pubkey
                .insert(user.pubkey.clone(), user.user_id.clone());
            state.users.insert(user.user_id.clone(), user);
        }
        for room in rooms {
            state.rooms.insert(room.room_id.clone(), room);
        }
        if state.trains.is_empty() {
            state.trains = init_trains();
        }
        state.next_user_id += 1;
        state
    }

    pub fn create_user(&mut self, pubkey: String) -> UserState {
        let user_id = format!("u_{}", self.next_user_id);
        self.next_user_id += 1;
        let user = UserState {
            user_id: user_id.clone(),
            pubkey: pubkey.clone(),
            display_name: None,
            position: Position {
                map_id: "street".to_string(),
                x: 0,
                y: 1,
            },
        };
        self.users_by_pubkey.insert(pubkey, user_id.clone());
        self.users.insert(user_id.clone(), user.clone());
        user
    }

    pub fn get_or_create_room(&mut self, room_id: &str, price_xmr: &str) -> RoomState {
        if let Some(room) = self.rooms.get(room_id) {
            return room.clone();
        }
        let room = RoomState {
            room_id: room_id.to_string(),
            owner_pubkey: None,
            price_xmr: price_xmr.to_string(),
            for_sale: true,
            access: AccessPolicy {
                mode: AccessMode::Open,
                list: Vec::new(),
            },
            display_name: None,
            door_color: None,
        };
        self.rooms.insert(room_id.to_string(), room.clone());
        room
    }

    pub fn add_connected_user(&mut self, user_id: &str, map_id: &str) {
        self.connected_users_by_map
            .entry(map_id.to_string())
            .or_default()
            .insert(user_id.to_string());
    }

    pub fn remove_connected_user(&mut self, user_id: &str, map_id: &str) {
        if let Some(entry) = self.connected_users_by_map.get_mut(map_id) {
            entry.remove(user_id);
            if entry.is_empty() {
                self.connected_users_by_map.remove(map_id);
            }
        }
    }

    pub fn move_connected_user(&mut self, user_id: &str, from_map: &str, to_map: &str) {
        if from_map == to_map {
            return;
        }
        self.remove_connected_user(user_id, from_map);
        self.add_connected_user(user_id, to_map);
    }
}

fn init_trains() -> Vec<TrainState> {
    let mut trains = Vec::new();
    let circumference = STREET_CIRCUMFERENCE_TILES as f64;
    let spacing = circumference / 4.0;
    let speed = 128.0;
    for id in 0..4u32 {
        trains.push(TrainState {
            id,
            x: spacing * id as f64,
            speed,
        });
    }
    trains
}
