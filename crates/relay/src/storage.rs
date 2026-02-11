use std::fs;
use std::path::{Path, PathBuf};

use crate::state::{RoomState, UserState};

pub struct Storage {
    base: PathBuf,
}

impl Storage {
    pub fn new(base: &str) -> anyhow::Result<Self> {
        let path = PathBuf::from(base);
        if !path.exists() {
            fs::create_dir_all(&path)?;
        }
        Ok(Self { base: path })
    }

    pub fn load_users(&self) -> anyhow::Result<Vec<UserState>> {
        self.load_json("users.json")
    }

    pub fn load_rooms(&self) -> anyhow::Result<Vec<RoomState>> {
        self.load_json("rooms.json")
    }

    pub fn save_users(&self, users: impl IntoIterator<Item = UserState>) -> anyhow::Result<()> {
        self.save_json("users.json", users)
    }

    pub fn save_rooms(&self, rooms: impl IntoIterator<Item = RoomState>) -> anyhow::Result<()> {
        self.save_json("rooms.json", rooms)
    }

    pub async fn save_users_async(&self, users: Vec<UserState>) -> anyhow::Result<()> {
        let base = self.base.clone();
        tokio::task::spawn_blocking(move || {
            let storage = Storage { base };
            storage.save_users(users)
        })
        .await??;
        Ok(())
    }

    pub async fn save_rooms_async(&self, rooms: Vec<RoomState>) -> anyhow::Result<()> {
        let base = self.base.clone();
        tokio::task::spawn_blocking(move || {
            let storage = Storage { base };
            storage.save_rooms(rooms)
        })
        .await??;
        Ok(())
    }

    fn load_json<T: serde::de::DeserializeOwned>(&self, file: &str) -> anyhow::Result<Vec<T>> {
        let path = self.base.join(file);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(path)?;
        if content.trim().is_empty() {
            return Ok(Vec::new());
        }
        let data = serde_json::from_str(&content)?;
        Ok(data)
    }

    fn save_json<T: serde::Serialize>(
        &self,
        file: &str,
        items: impl IntoIterator<Item = T>,
    ) -> anyhow::Result<()> {
        let path = self.base.join(file);
        let items: Vec<T> = items.into_iter().collect();
        let content = serde_json::to_string_pretty(&items)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn base_path(&self) -> &Path {
        &self.base
    }
}
