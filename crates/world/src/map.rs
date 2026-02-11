use serde::{Deserialize, Serialize};

use crate::doors::{room_door_position, street_door_side, RoomSide};
use crate::monorail::{is_station_door, STATION_DOOR_Y_BOTTOM, STATION_DOOR_Y_TOP};

pub const STREET_HEIGHT: i32 = 16;
pub const ROOM_WIDTH: i32 = 32;
pub const ROOM_HEIGHT: i32 = 16;
// pub const STREET_CIRCUMFERENCE_TILES: i64 = 65_536_000;
pub const STREET_CIRCUMFERENCE_TILES: i64 = 4096;
pub const STATION_WIDTH: i32 = 32;
pub const STATION_HEIGHT: i32 = 16;
pub const TRAIN_WIDTH: i32 = 32;
pub const TRAIN_HEIGHT: i32 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tile {
    Wall,
    Door,
    StationDoor,
    Customizer,
    Floor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub map_id: String,
    pub x: i32,
    pub y: i32,
}

pub fn street_tile(x: i32, y: i32) -> Tile {
    if is_station_door(x, y) {
        return Tile::StationDoor;
    }
    if y == 0 || y == STREET_HEIGHT - 1 {
        if street_door_side(x, y).is_some() {
            Tile::Door
        } else {
            Tile::Wall
        }
    } else {
        Tile::Floor
    }
}

pub fn room_tile(x: i32, y: i32, side: RoomSide) -> Tile {
    if (x, y) == room_customizer_position() {
        return Tile::Customizer;
    }
    if x == 0 || x == ROOM_WIDTH - 1 || y == 0 || y == ROOM_HEIGHT - 1 {
        let (door_x, door_y) = room_door_position(side);
        if x == door_x && y == door_y {
            Tile::Door
        } else {
            Tile::Wall
        }
    } else {
        Tile::Floor
    }
}

pub fn room_customizer_position() -> (i32, i32) {
    (1, 1)
}

pub fn station_tile(x: i32, y: i32) -> Tile {
    if x == 0 || x == STATION_WIDTH - 1 || y == 0 || y == STATION_HEIGHT - 1 {
        let door_x = STATION_WIDTH / 2;
        if x == door_x && (y == 0 || y == STATION_HEIGHT - 1) {
            Tile::Door
        } else {
            Tile::Wall
        }
    } else {
        Tile::Floor
    }
}

pub fn station_entry_position() -> (i32, i32) {
    let door_x = STATION_WIDTH / 2;
    let door_y = STATION_HEIGHT - 2;
    (door_x, door_y)
}

pub fn station_entry_position_for_street_y(street_y: i32) -> (i32, i32) {
    let door_x = STATION_WIDTH / 2;
    let door_y = if street_y == STATION_DOOR_Y_TOP {
        1
    } else if street_y == STATION_DOOR_Y_BOTTOM {
        STATION_HEIGHT - 2
    } else {
        STATION_HEIGHT - 2
    };
    (door_x, door_y)
}

pub fn train_tile(x: i32, y: i32) -> Tile {
    if x == 0 || x == TRAIN_WIDTH - 1 || y == 0 || y == TRAIN_HEIGHT - 1 {
        Tile::Wall
    } else {
        Tile::Floor
    }
}
