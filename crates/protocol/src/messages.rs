use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    #[serde(rename = "type")]
    pub message_type: String,
    pub id: String,
    pub ts: i64,
    pub sig: Option<String>,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignableEnvelope {
    #[serde(rename = "type")]
    pub message_type: String,
    pub id: String,
    pub ts: i64,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub map_id: String,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerHello {
    pub server_version: String,
    pub challenge: String,
    pub fee_config: DevFeeConfig,
    pub room_price_xmr: String,
    pub username_fee_xmr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientAuth {
    pub pubkey: String,
    pub challenge_sig: String,
    pub client_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerWelcome {
    pub client_id: String,
    pub display_name: Option<String>,
    pub position: Position,
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientMove {
    pub dir: Direction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientChat {
    pub scope: Option<ChatScope>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientCommand {
    pub name: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientRoomAccessUpdate {
    pub room_id: String,
    pub mode: AccessMode,
    pub list: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientHeartbeat {
    pub nonce: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerState {
    pub position: Position,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerMapChange {
    pub map_id: String,
    pub position: Position,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerChat {
    pub from: String,
    pub display_name: Option<String>,
    pub text: String,
    pub scope: ChatScope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerNearby {
    pub users: Vec<NearbyUser>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NearbyUser {
    pub id: String,
    pub display_name: Option<String>,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerWho {
    pub users: Vec<WhoUser>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhoUser {
    pub id: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerRoomInfo {
    pub room_id: String,
    pub owner: Option<String>,
    pub price_xmr: String,
    pub for_sale: bool,
    pub access: AccessPolicy,
    pub display_name: Option<String>,
    pub door_color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerTxUpdate {
    pub tx_id: String,
    pub status: String,
    pub confirmations: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerNotice {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerTrainState {
    pub trains: Vec<TrainInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainInfo {
    pub id: u32,
    pub x: f64,
    pub clockwise: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerHeartbeat {
    pub nonce: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevFeeConfig {
    pub mode: String,
    pub value: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessPolicy {
    pub mode: AccessMode,
    pub list: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccessMode {
    Open,
    Whitelist,
    Blacklist,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatScope {
    Local,
    Whisper,
    Room,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}
