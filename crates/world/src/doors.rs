use serde::{Deserialize, Serialize};

use crate::map::{ROOM_HEIGHT, ROOM_WIDTH, STREET_HEIGHT};

pub const DOOR_SPACING: i32 = 6;
pub const DOOR_OFFSET: i32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoomSide {
    North,
    South,
}

impl RoomSide {
    pub fn as_str(&self) -> &'static str {
        match self {
            RoomSide::North => "north",
            RoomSide::South => "south",
        }
    }
}

pub fn street_door_side(x: i32, y: i32) -> Option<RoomSide> {
    let offset = x.rem_euclid(DOOR_SPACING);
    if y == 0 && offset == 0 {
        Some(RoomSide::North)
    } else if y == STREET_HEIGHT - 1 && offset == DOOR_OFFSET {
        Some(RoomSide::South)
    } else {
        None
    }
}

pub fn room_id_for_door(side: RoomSide, street_x: i32) -> String {
    format!("{}:{}", side.as_str(), street_x)
}

pub fn distance_to_nearest_door(x: i32) -> i32 {
    let offset = x.rem_euclid(DOOR_SPACING);
    let dist_to_zero = offset.min(DOOR_SPACING - offset);
    let dist_to_offset = (offset - DOOR_OFFSET).abs();
    let dist_to_offset = dist_to_offset.min(DOOR_SPACING - dist_to_offset);
    dist_to_zero.min(dist_to_offset)
}

pub fn room_map_id(room_id: &str) -> String {
    format!("room/{}", room_id)
}

pub fn parse_room_id(room_id: &str) -> Option<(RoomSide, i32)> {
    let mut parts = room_id.split(':');
    let side = parts.next()?;
    let x_part = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let street_x: i32 = x_part.parse().ok()?;
    let side = match side {
        "north" => RoomSide::North,
        "south" => RoomSide::South,
        _ => return None,
    };
    Some((side, street_x))
}

pub fn parse_room_map_id(map_id: &str) -> Option<(RoomSide, i32)> {
    let room_id = map_id.strip_prefix("room/")?;
    parse_room_id(room_id)
}

pub fn room_door_position(side: RoomSide) -> (i32, i32) {
    let door_x = ROOM_WIDTH / 2;
    let door_y = match side {
        RoomSide::North => ROOM_HEIGHT - 1,
        RoomSide::South => 0,
    };
    (door_x, door_y)
}

pub fn room_entry_position(side: RoomSide) -> (i32, i32) {
    let door_x = ROOM_WIDTH / 2;
    let door_y = match side {
        RoomSide::North => ROOM_HEIGHT - 2,
        RoomSide::South => 1,
    };
    (door_x, door_y)
}

pub fn street_entry_position(side: RoomSide, street_x: i32) -> (i32, i32) {
    let y = match side {
        RoomSide::North => 1,
        RoomSide::South => STREET_HEIGHT - 2,
    };
    (street_x, y)
}
