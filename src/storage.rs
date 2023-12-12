use std::{collections::HashMap, env, fs, path::Path};

use log::{error, warn};
use uuid::Uuid;

use crate::models::{Light, LightingResponse, Room};

/// Reads and syncs with `rooms.json` in `RIZ_STORAGE_PATH`
///
/// Expected to be wrapped by a [std::sync::Mutex], then wrapped
/// with a [actix_web::web::Data], and cloned to each request
///
/// NB: All `&mut` methods update the contents of `rooms.json`
///
/// # Examples
///
/// ```
/// use std::sync::Mutex;
/// use actix_web::web::Data;
/// use riz::Storage;
///
/// let storage = Data::new(Mutex::new(Storage::new()));
/// ```
///
pub struct Storage {
    rooms: HashMap<Uuid, Room>,
    file_path: String,
}

impl Storage {
    /// Create a new Stoage object (should only do this once)
    pub fn new() -> Self {
        let path = env::var("RIZ_STORAGE_PATH").unwrap_or(".".to_string());
        if let Some(file_path) = Path::new(&path).join("rooms.json").to_str() {
            match fs::read_to_string(file_path) {
                Ok(content) => {
                    if let Ok(prev) = serde_json::from_str(&content) {
                        Storage {
                            rooms: prev,
                            file_path: String::from(file_path),
                        }
                    } else {
                        warn!("Failed to decode previous data");
                        Storage {
                            rooms: HashMap::new(),
                            file_path: String::from(file_path),
                        }
                    }
                }
                Err(_) => Storage {
                    rooms: HashMap::new(),
                    file_path: String::from(file_path),
                },
            }
        } else {
            warn!("Invalid storage file path: {}", path);
            Storage {
                rooms: HashMap::new(),
                file_path: String::from("./rooms.json"),
            }
        }
    }

    /// Write the contents of self.rooms to rooms.json
    fn write(&self) {
        if let Ok(contents) = serde_json::to_string(&self.rooms) {
            match fs::write(&self.file_path, contents) {
                Err(e) => error!("Failed to write JSON: {:?}", e),
                _ => {}
            }
        } else {
            error!("Failed to dump JSON");
        }
    }

    /// Create a new room
    pub fn new_room(&mut self, room: Room) -> Result<Uuid, String> {
        let mut id = Uuid::new_v4();
        while self.rooms.contains_key(&id) {
            id = Uuid::new_v4();
        }
        self.rooms.insert(id, room);
        self.write();
        Ok(id)
    }

    /// Create a new light in the room
    pub fn new_light(&mut self, room: &Uuid, light: Light) -> Result<Uuid, String> {
        if let Some(entry) = self.rooms.get_mut(room) {
            let id = entry.new_light(light);
            self.write();
            Ok(id)
        } else {
            Err(format!("Not found: {}", room))
        }
    }

    /// Read a room by ID (returns clone)
    pub fn read(&self, room: &Uuid) -> Option<Room> {
        match self.rooms.get(room) {
            Some(room) => Some(room.clone()),
            None => None,
        }
    }

    /// Updates non-light attributes (currently just name)
    pub fn update_room(&mut self, id: &Uuid, room: &Room) -> Result<(), String> {
        if let Some(entry) = self.rooms.get_mut(id) {
            if entry.update(room) {
                self.write();
                Ok(())
            } else {
                Err(format!("No update found (matching data): {}", id))
            }
        } else {
            Err(format!("Not found: {}", id))
        }
    }

    /// Update non-lighting attributes of the light in the room (name, ip)
    pub fn update_light(
        &mut self,
        id: &Uuid,
        light_id: &Uuid,
        light: &Light,
    ) -> Result<(), String> {
        if let Some(room) = self.rooms.get_mut(id) {
            room.update_light(light_id, light)?;
            self.write();
            Ok(())
        } else {
            Err(format!("Not found: {}", id))
        }
    }

    /// Remove a room
    pub fn delete_room(&mut self, room: &Uuid) -> Result<(), String> {
        match self.rooms.remove(room) {
            Some(_) => {
                self.write();
                Ok(())
            }
            None => Err(format!("Not found: {}", room)),
        }
    }

    /// Remove a light in a room
    pub fn delete_light(&mut self, room: &Uuid, light: &Uuid) -> Result<(), String> {
        match self.rooms.get_mut(room) {
            Some(rm) => {
                rm.delete_light(light)?;
                self.write();
                Ok(())
            }
            None => Err(format!("No such room: {}", room)),
        }
    }

    /// List room IDs
    pub fn list(&self) -> Result<Vec<&Uuid>, String> {
        Ok(self.rooms.keys().collect())
    }

    /// Process the response of a lighting request
    pub fn process_reply(&mut self, resp: &LightingResponse) {
        let mut any_update = false;
        for room in self.rooms.values_mut() {
            let room_update = room.process_reply(&resp);
            any_update = any_update || room_update;
        }

        if any_update {
            self.write();
        }
    }
}
