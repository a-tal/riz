use std::{collections::HashMap, env, fs, net::Ipv4Addr, path::Path};

use ipnet::Ipv4Net;
use log::{error, warn};
use uuid::Uuid;

use crate::{
    models::{Light, LightingResponse, Room},
    Error, Result,
};

const STORAGE_ENV_KEY: &str = "RIZ_STORAGE_PATH";

/// Reads and syncs with `rooms.json` in `RIZ_STORAGE_PATH` (env var)
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
#[derive(Default, Debug)]
pub struct Storage {
    rooms: HashMap<Uuid, Room>,
    file_path: String,
}

impl Storage {
    /// Create a new Stoage object (should only do this once)
    pub fn new() -> Self {
        let file_path = Self::get_storage_path();
        let mut rooms = Self::read_json(&file_path);

        for (id, room) in rooms.iter_mut() {
            room.link(id);
        }

        Storage { rooms, file_path }
    }

    fn read_json(file_path: &str) -> HashMap<Uuid, Room> {
        match fs::read_to_string(file_path) {
            Ok(content) => {
                if let Ok(prev) = serde_json::from_str(&content) {
                    prev
                } else {
                    warn!("Failed to decode previous data");
                    HashMap::new()
                }
            }
            Err(_) => HashMap::new(),
        }
    }

    fn get_storage_path() -> String {
        let path = env::var(STORAGE_ENV_KEY).unwrap_or(".".to_string());
        if let Some(file_path) = Path::new(&path).join("rooms.json").to_str() {
            file_path
        } else {
            warn!("Invalid storage file path: {}", path);
            "./rooms.json"
        }
        .to_string()
    }

    /// Write the contents of self.rooms to rooms.json
    fn write(&self) {
        if let Ok(contents) = serde_json::to_string(&self.rooms) {
            if let Err(e) = fs::write(&self.file_path, contents) {
                error!("Failed to write JSON: {:?}", e);
            }
        } else {
            error!("Failed to dump JSON");
        }
    }

    /// Create a new room
    ///
    /// # Errors
    ///   [Error::InvalidIP] if any light in the new room has an invalid IP address
    ///
    pub fn new_room(&mut self, room: Room) -> Result<Uuid> {
        let mut id = Uuid::new_v4();
        while self.rooms.contains_key(&id) {
            id = Uuid::new_v4();
        }

        // ensure any lights ips in the new room are valid (should be empty...)
        self.validate_room(&room)?;

        let mut room = room;
        room.link(&id);

        self.rooms.insert(id, room);
        self.write();
        Ok(id)
    }

    /// Create a new light in the room
    pub fn new_light(&mut self, room: &Uuid, light: Light) -> Result<Uuid> {
        self.validate_light(&light)?;
        if let Some(entry) = self.rooms.get_mut(room) {
            let id = entry.new_light(light)?;
            self.write();
            Ok(id)
        } else {
            Err(Error::RoomNotFound(*room))
        }
    }

    /// Read a room by ID (returns clone)
    pub fn read(&self, room: &Uuid) -> Option<Room> {
        self.rooms.get(room).cloned()
    }

    /// Updates non-light attributes (currently just name)
    pub fn update_room(&mut self, id: &Uuid, room: &Room) -> Result<()> {
        if let Some(entry) = self.rooms.get_mut(id) {
            if entry.update(room) {
                self.write();
                Ok(())
            } else {
                Err(Error::NoChangeRoom(*id))
            }
        } else {
            Err(Error::RoomNotFound(*id))
        }
    }

    /// Update non-lighting attributes of the light in the room (name, ip)
    pub fn update_light(&mut self, id: &Uuid, light_id: &Uuid, light: &Light) -> Result<()> {
        if let Some(room) = self.rooms.get_mut(id) {
            room.update_light(light_id, light)?;
            self.write();
            Ok(())
        } else {
            Err(Error::light_not_found(id, light_id))
        }
    }

    /// Remove a room
    pub fn delete_room(&mut self, room: &Uuid) -> Result<()> {
        match self.rooms.remove(room) {
            Some(_) => {
                self.write();
                Ok(())
            }
            None => Err(Error::RoomNotFound(*room)),
        }
    }

    /// Remove a light in a room
    pub fn delete_light(&mut self, room: &Uuid, light: &Uuid) -> Result<()> {
        match self.rooms.get_mut(room) {
            Some(rm) => {
                rm.delete_light(light)?;
                self.write();
                Ok(())
            }
            None => Err(Error::RoomNotFound(*room)),
        }
    }

    /// List room IDs
    pub fn list(&self) -> Result<Vec<&Uuid>> {
        Ok(self.rooms.keys().collect())
    }

    /// Process the response of a lighting request
    pub fn process_reply(&mut self, resp: &LightingResponse) {
        let mut any_update = false;
        for room in self.rooms.values_mut() {
            let room_update = room.process_reply(resp);
            any_update = any_update || room_update;
        }

        if any_update {
            self.write();
        }
    }

    /// Check if all lights in the room are valid and unique
    fn validate_room(&self, room: &Room) -> Result<()> {
        if let Some(lights) = room.list() {
            for light_id in lights {
                if let Some(light) = room.read(light_id) {
                    self.validate_light(light)?;
                }
            }
        }
        Ok(())
    }

    /// Check if the light's ip is valid and unqiue
    fn validate_light(&self, light: &Light) -> Result<()> {
        self.validate_ip(&light.ip())
    }

    /// Check if the IP is valid and unique
    fn validate_ip(&self, ip: &Ipv4Addr) -> Result<()> {
        // || ip.is_benchmarking() can be added once stable
        if ip.is_documentation() {
            return self.unique_ip(ip);
        }

        if ip.is_link_local() || ip.is_loopback() {
            return Err(Error::invalid_ip(ip, "a local ip"));
        }

        if ip.is_unspecified() {
            return Err(Error::invalid_ip(ip, "unspecified"));
        }

        if ip.is_broadcast() {
            return Err(Error::invalid_ip(ip, "a broadcast address"));
        }

        if ip.is_multicast() {
            return Err(Error::invalid_ip(ip, "a multicast address"));
        }

        // can add when when stable
        // if ip.is_reserved() {
        //     return Err(Error::invalid_ip(ip, "a reserved ip"));
        // }

        if !ip.is_private() {
            return Err(Error::invalid_ip(ip, "a public ip"));
        }

        // check if this IP is a subnet broadcast or network address
        if let Some(net) = classful_network(ip) {
            // NB: because we are probably behind docker, we can't
            //     really tell what our local network is, without
            //     probing around... which we probably shouldn't do.
            //     otherwise, it would be possible to limit the IPs
            //     to the actual connected networks. but as we've
            //     already limited them to private IPs this is fine.
            //     it won't correctly pick up classless setups though,
            //     again because docker. ¯\_(ツ)_/¯ oh well

            if *ip == net.network() {
                return Err(Error::invalid_ip(ip, "the subnet's network address"));
            }

            if *ip == net.broadcast() {
                return Err(Error::invalid_ip(ip, "the subnet's broadcast address"));
            }

            return self.unique_ip(ip);
        }

        // this can't actually happen...
        Err(Error::invalid_ip(ip, "unknown"))
    }

    /// Check if the IP is unique
    fn unique_ip(&self, ip: &Ipv4Addr) -> Result<()> {
        for room in self.rooms.values() {
            if let Some(lights) = room.list() {
                for light_id in lights {
                    if let Some(light) = room.read(light_id) {
                        if *ip == light.ip() {
                            return Err(Error::invalid_ip(ip, "already known"));
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

fn classful_network(ip: &Ipv4Addr) -> Option<Ipv4Net> {
    match ip.octets()[0] {
        (1..=126) => Some(Ipv4Net::new(*ip, 8).unwrap()),
        (128..=191) => Some(Ipv4Net::new(*ip, 16).unwrap()),
        (192..=223) => Some(Ipv4Net::new(*ip, 24).unwrap()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use rand::{distributions::Alphanumeric, Rng};
    use std::{env, panic, str::FromStr, vec};

    use super::*;

    /// Run the closure test with a new temp test storage, and clean up after
    fn test_storage<T>(test: T) -> ()
    where
        T: FnOnce() -> () + panic::UnwindSafe,
    {
        let s: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(12)
            .map(char::from)
            .collect();

        let mut base = env::temp_dir();
        base.push(s);
        env::set_var(STORAGE_ENV_KEY, base.clone());

        let res = panic::catch_unwind(|| test());

        fs::remove_dir_all(base).unwrap_or_else(|_| error!("failed to clean up tmp storage"));

        assert!(res.is_ok())
    }

    #[test]
    fn unique_ips_same_room() {
        let mut room = Room::new("test");
        let ip = Ipv4Addr::from_str("192.0.2.3").unwrap();
        let light = Light::new(ip, Some("bulb"));

        assert!(room.new_light(light.clone()).is_ok());
        let res = room.new_light(light);

        assert_eq!(res, Err(Error::invalid_ip(&ip, "already known")));
    }

    #[test]
    fn unique_ips_different_rooms() {
        test_storage(|| {
            let ip = Ipv4Addr::from_str("192.0.2.3").unwrap();

            let mut room = Room::new("test");
            let light = Light::new(ip, Some("bulb"));
            room.new_light(light.clone()).unwrap();

            let mut room2 = Room::new("test");
            room2.new_light(light).unwrap();

            let mut storage = Storage::new();
            assert!(storage.new_room(room).is_ok());

            let res = storage.new_room(room2);
            assert_eq!(res, Err(Error::invalid_ip(&ip, "already known")));
        })
    }

    #[test]
    fn new_light_unique_ip() {
        test_storage(|| {
            let ip = Ipv4Addr::from_str("192.0.2.3").unwrap();

            let mut room = Room::new("test");
            let light = Light::new(ip, Some("bulb"));
            room.new_light(light.clone()).unwrap();

            let mut storage = Storage::new();
            let room_id = storage.new_room(room).unwrap();

            let res = storage.new_light(&room_id, light);
            assert_eq!(res, Err(Error::invalid_ip(&ip, "already known")));
        })
    }

    #[test]
    fn invalid_ips_denied() {
        test_storage(|| {
            let tests = vec![
                ("8.8.8.8", "a public ip"),
                ("127.0.0.1", "a local ip"),
                ("0.0.0.0", "unspecified"),
                ("255.255.255.255", "a broadcast address"),
                ("224.224.224.224", "a multicast address"),
                // ("240.240.240.240", "a reserved ip"),
                ("192.168.1.0", "the subnet's network address"),
                ("172.16.255.255", "the subnet's broadcast address"),
            ];

            for (ip, reason) in tests {
                let ip = Ipv4Addr::from_str(ip).unwrap();

                let mut room = Room::new("test");
                let light = Light::new(ip, None);
                room.new_light(light).unwrap();

                let mut storage = Storage::new();
                let res = storage.new_room(room);

                assert_eq!(res, Err(Error::invalid_ip(&ip, reason)));
            }
        })
    }

    #[test]
    fn valid_ips_allowed() {
        test_storage(|| {
            let tests = vec!["10.1.2.3", "192.168.1.25", "172.16.0.17"];

            for ip in tests {
                let ip = Ipv4Addr::from_str(ip).unwrap();

                let mut room = Room::new("test");
                let light = Light::new(ip, None);
                room.new_light(light).unwrap();

                let mut storage = Storage::new();
                let res = storage.new_room(room);

                assert!(res.is_ok());
            }
        })
    }
}
