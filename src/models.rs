//! Riz models

use std::collections::HashMap;
use std::net::{Ipv4Addr, UdpSocket};
use std::result::Result as StdResult;
use std::str::FromStr;
use std::time::Duration;

use log::debug;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{Error, Result};

/// Rooms group lights logically to allow for batched actions
///
/// NB: They don't have to be the same as configured by the Wiz app
///
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct Room {
    #[schema(min_length = 1, max_length = 100)]
    name: String,
    #[schema(max_items = 100)]
    lights: Option<HashMap<Uuid, Light>>,

    #[serde(skip)]
    id: Uuid,
    #[serde(skip)]
    linked: bool,
}

impl Room {
    /// Create a new room with some name and no lights
    pub fn new(name: &str) -> Self {
        Room {
            name: String::from(name),
            lights: None,
            id: Uuid::new_v4(),
            linked: false,
        }
    }

    /// Link the id to this Room for self-reference
    ///
    /// Can only be called once
    ///
    /// # Panics
    ///   If called more than once
    ///
    pub fn link(&mut self, id: &Uuid) {
        if self.linked {
            panic!("refusing to overwrite id!")
        }
        self.id = *id;
        self.linked = true;
    }

    /// Ask all bulbs in this room for their current status
    ///
    /// # Returns
    ///   a [Result] of:
    ///   (unordered) [Vec] of [LightingResponse] from all bulbs on success
    ///   and [Error] if there's any error getting status from any bulb
    ///
    pub fn get_status(&mut self) -> Result<Vec<LightingResponse>> {
        let mut resp = Vec::new();
        if let Some(lights) = &mut self.lights {
            for light in lights.values_mut() {
                let status = light.get_status()?;
                resp.push(LightingResponse::status(light.ip, status));
            }
        }
        Ok(resp)
    }

    /// Store a newly created [Light] in this room
    ///
    /// Will generate a new [Uuid] and store the [Light] in this lights.
    ///
    /// # Returns
    ///   the newly created [Uuid] for the [Light]
    ///
    pub fn new_light(&mut self, light: Light) -> Result<Uuid> {
        self.validate_light(&light, None)?;
        let mut id = Uuid::new_v4();
        if let Some(lights) = self.lights.as_mut() {
            while lights.contains_key(&id) {
                id = Uuid::new_v4();
            }
            lights.insert(id, light);
        } else {
            self.lights = Some(HashMap::from([(id, light)]));
        }
        Ok(id)
    }

    /// Removes a light from the room's lights
    ///
    /// # Returns
    ///   [Err] [String] when unable to find the light ID or no lights
    ///
    pub fn delete_light(&mut self, light: &Uuid) -> Result<()> {
        if let Some(lights) = self.lights.as_mut() {
            match lights.remove(light) {
                Some(_) => Ok(()),
                None => Err(Error::light_not_found(&self.id, light)),
            }
        } else {
            Err(Error::RoomNotFound(self.id))
        }
    }

    /// Update the non-lighting settings of a light bulb
    ///
    /// # Examples
    ///
    /// ```
    /// use std::str::FromStr;
    /// use std::net::Ipv4Addr;
    /// use riz::models::{Room, Light};
    ///
    /// let ip1 = Ipv4Addr::from_str("10.1.2.3").unwrap();
    /// let ip2 = Ipv4Addr::from_str("10.1.2.4").unwrap();
    ///
    /// let mut room = Room::new("test");
    ///
    /// let light = Light::new(ip1, Some("foo"));
    /// let light_id = room.new_light(light).unwrap();
    ///
    /// let read = room.read(&light_id).unwrap();
    /// assert_eq!(read.name(), Some("foo"));
    /// assert_eq!(read.ip(), ip1);
    ///
    /// room.update_light(&light_id, &Light::new(ip2, Some("bar"))).unwrap();
    ///
    /// let read = room.read(&light_id).unwrap();
    /// assert_eq!(read.name(), Some("bar"));
    /// assert_eq!(read.ip(), ip2);
    /// ```
    ///
    /// # Returns
    ///   [Err] [String] if either room or light id is not known
    ///
    pub fn update_light(&mut self, id: &Uuid, light: &Light) -> Result<()> {
        if let Some(lights) = self.lights.as_mut() {
            match lights.get_mut(id) {
                Some(l) => {
                    if l.update(light) {
                        Ok(())
                    } else {
                        Err(Error::no_change_light(&self.id, id))
                    }
                }
                None => Err(Error::light_not_found(&self.id, id)),
            }
        } else {
            Err(Error::NoLights(self.id))
        }
    }

    /// List all lights in this room, if any
    ///
    /// # Returns
    ///   [Vec] of &[Uuid]; valid [Light] IDs
    ///
    /// # Examples
    ///
    /// ```
    /// use std::str::FromStr;
    /// use std::net::Ipv4Addr;
    /// use riz::models::{Room, Light};
    ///
    /// let mut room = Room::new("test");
    /// assert!(room.list().is_none());
    ///
    /// let light = Light::new(Ipv4Addr::from_str("10.1.2.3").unwrap(), None);
    /// let light_id = room.new_light(light).unwrap();
    ///
    /// let ids = room.list().unwrap();
    /// assert_eq!(*ids.iter().next().unwrap(), &light_id);
    /// ```
    ///
    pub fn list(&self) -> Option<Vec<&Uuid>> {
        self.lights.as_ref().map(|lights| lights.keys().collect())
    }

    /// Read a light in this room by ID
    ///
    /// # Returns
    ///   [Option] of &[Light] if the &[Uuid] if known
    ///
    pub fn read(&self, light: &Uuid) -> Option<&Light> {
        match &self.lights {
            Some(lights) => lights.get(light),
            None => None,
        }
    }

    /// Read a light in this room by ID as a mutable reference
    ///
    /// # Returns
    ///   [Option] of &mut [Light] if the &[Uuid] is known
    ///
    pub fn read_mut(&mut self, light: &Uuid) -> Option<&mut Light> {
        if let Some(lights) = self.lights.as_mut() {
            lights.get_mut(light)
        } else {
            None
        }
    }

    /// Process a reply from a lighting request for bulbs in this room
    ///
    /// # Returns
    ///   [bool] of if any of this room's lights were updated
    ///
    pub fn process_reply(&mut self, resp: &LightingResponse) -> bool {
        let mut any_update = false;
        if let Some(lights) = self.lights.as_mut() {
            for light in lights.values_mut() {
                let light_update = light.process_reply(resp);
                any_update = any_update || light_update;
            }
        }
        any_update
    }

    /// Accessor for this room's name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Update our (non-light) attributes from the other instance
    ///
    /// # Examples
    ///
    /// ```
    /// use riz::models::Room;
    ///
    /// let mut room = Room::new("foo");
    /// let other = Room::new("bar");
    /// assert!(room.update(&other));
    /// assert_eq!(room.name(), "bar");
    /// ```
    ///
    pub fn update(&mut self, other: &Self) -> bool {
        if self.name == other.name {
            return false;
        }
        self.name = other.name.clone();
        true
    }

    fn validate_light(&self, light: &Light, light_id: Option<&Uuid>) -> Result<()> {
        let ip = light.ip();
        if let Some(lights) = self.lights.as_ref() {
            for (id, known) in lights {
                if Some(id) == light_id {
                    continue;
                }
                if known.ip() == ip {
                    return Err(Error::invalid_ip(&ip, "already known"));
                }
            }
        }
        Ok(())
    }
}

/// Lights are grouped per room, or used individually by the CLI
///
/// # Examples
///
/// ```
/// use std::net::Ipv4Addr;
/// use std::str::FromStr;
/// use riz::models::Light;
///
/// let light = Light::new(Ipv4Addr::from_str("10.1.2.3").unwrap(), None);
/// assert!(light.status().is_none());
/// ```
///
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct Light {
    /// IPv4 address for the light, ideally statically assigned
    #[schema(
      min_length = 1,
      max_length = 15,
      value_type = String,
      example = "192.168.1.50",
      pattern = r"^(((1[\d]{0,2})|(2([0-4]?[\d]|5[0-5]))|([3-9]?[\d])|[\d])\.){0,3}((1[\d]{0,2})|(2([0-4]?[\d]|5[0-5]))|([3-9]?[\d])|[\d])$")]
    ip: Ipv4Addr,

    /// Name of light, arbitrary (user supplied)
    #[schema(min_length = 1, max_length = 100)]
    name: Option<String>,

    /// Last known status, if any
    status: Option<LightStatus>,
}

impl Light {
    /// Create a new optionally named light with no known status
    pub fn new(ip: Ipv4Addr, name: Option<&str>) -> Self {
        Light {
            ip,
            name: name.map(String::from),
            status: None,
        }
    }

    /// Accessor for this bulb's IP address
    pub fn ip(&self) -> Ipv4Addr {
        self.ip
    }

    /// Accessor for this bulb's name
    pub fn name(&self) -> Option<&str> {
        match &self.name {
            Some(s) => Some(s),
            None => None,
        }
    }

    /// Accessor for this bulb's last known status
    pub fn status(&self) -> Option<&LightStatus> {
        self.status.as_ref()
    }

    /// Ask the bulb for its status
    ///
    /// Note that this is not the same as accessing the last known
    /// status for the bulb, this method sends a new request for data,
    ///
    /// If you want to update the last known state, you can pass the
    /// newly fetched status into [Self::process_reply]
    ///
    pub fn get_status(&self) -> Result<LightStatus> {
        let resp = self.udp_response(&json!({"method": "getPilot"}))?;

        let status: BulbStatus = match serde_json::from_value(resp) {
            Ok(v) => v,
            Err(e) => return Err(Error::JsonLoad(e)),
        };
        let status = LightStatus::from(&status);
        Ok(status)
    }

    /// Set new lighting settings on this bulb
    ///
    /// Does not update self.status, you can pass the response back
    /// into [Self::process_reply] if you want to update the internal state
    ///
    pub fn set(&self, payload: &Payload) -> Result<LightingResponse> {
        if payload.is_valid() {
            match serde_json::to_value(payload) {
                Ok(msg) => match self.udp_response(&json!({
                  "method": "setPilot",
                  "params": msg,
                })) {
                    Ok(v) => {
                        debug!("udp response: {:?}", v);
                        Ok(LightingResponse::payload(self.ip, payload.clone()))
                    }
                    Err(e) => Err(e),
                },
                Err(e) => Err(Error::JsonDump(e)),
            }
        } else {
            Err(Error::NoAttribute)
        }
    }

    /// Set the [PowerMode] for the light
    ///
    /// Works in the same fashion as [Self::set], where the action does not
    /// mutate internal state. You can pass the response from this method
    /// to [Self::process_reply] if you want to update this bulb's status
    ///
    pub fn set_power(&self, power: &PowerMode) -> Result<LightingResponse> {
        match power {
            PowerMode::On => self.toggle_power(true),
            PowerMode::Off => self.toggle_power(false),
            PowerMode::Reboot => self.power_cycle(),
        }
    }

    fn toggle_power(&self, powered: bool) -> Result<LightingResponse> {
        self.udp_response(&json!({"method": "setState","params": { "state": powered }}))?;
        Ok(if powered {
            LightingResponse::power(self.ip, PowerMode::On)
        } else {
            LightingResponse::power(self.ip, PowerMode::Off)
        })
    }

    fn power_cycle(&self) -> Result<LightingResponse> {
        self.udp_response(&json!({"method": "reboot"}))?;
        Ok(LightingResponse::power(self.ip, PowerMode::Reboot))
    }

    /// Update this light's non-lighting attributes
    fn update(&mut self, other: &Self) -> bool {
        let mut any_update = false;
        if self.name != other.name {
            self.name = other.name.clone();
            any_update = true;
        }

        if self.ip != other.ip {
            self.ip = other.ip;
            any_update = true;
        }

        any_update
    }

    /// Update the internal state with the response of some command
    pub fn process_reply(&mut self, resp: &LightingResponse) -> bool {
        if resp.ip == self.ip {
            match &resp.response {
                LightingResponseType::Payload(payload) => self.update_status_from_payload(payload),
                LightingResponseType::Power(power) => self.update_status_from_power(power),
                LightingResponseType::Status(status) => self.update_status(status),
            }
            true
        } else {
            false
        }
    }

    fn update_status(&mut self, status: &LightStatus) {
        if let Some(known) = &mut self.status {
            known.update(status);
        } else {
            self.status = Some(status.clone());
        }
    }

    fn update_status_from_payload(&mut self, payload: &Payload) {
        if let Some(status) = &mut self.status {
            status.update_from_payload(payload);
        } else {
            self.status = Some(LightStatus::from(payload));
        }
    }

    fn update_status_from_power(&mut self, power: &PowerMode) {
        if let Some(status) = &mut self.status {
            status.update_from_power(power);
        } else {
            self.status = Some(LightStatus::from(power));
        }
    }

    fn udp_response(&self, msg: &Value) -> Result<Value> {
        // dump the control message to string
        let msg = match serde_json::to_string(&msg) {
            Ok(v) => v,
            Err(e) => return Err(Error::JsonDump(e)),
        };

        // get some udp socket from the os
        let socket = match UdpSocket::bind("0.0.0.0:0") {
            Ok(s) => s,
            Err(e) => return Err(Error::socket("bind", e)),
        };

        // set a 1 second read and write timeout
        match socket.set_write_timeout(Some(Duration::new(1, 0))) {
            Ok(_) => {}
            Err(e) => return Err(Error::socket("set_write_timeout", e)),
        };

        match socket.set_read_timeout(Some(Duration::new(1, 0))) {
            Ok(_) => {}
            Err(e) => return Err(Error::socket("set_read_timeout", e)),
        };

        // connect to the remote bulb at their standard port
        match socket.connect(format!("{}:38899", self.ip)) {
            Ok(_) => {}
            Err(e) => return Err(Error::socket("connect", e)),
        }

        // send the control message
        match socket.send(msg.as_bytes()) {
            Ok(_) => {}
            Err(e) => return Err(Error::socket("send", e)),
        };

        // declare a buffer of the max message size
        let mut buffer = [0; 4096];
        let bytes = match socket.recv(&mut buffer) {
            Ok(b) => b,
            Err(e) => return Err(Error::socket("receive", e)),
        };

        // Redeclare `buffer` as String of the received bytes
        let buffer = match String::from_utf8(buffer[..bytes].to_vec()) {
            Ok(s) => s,
            Err(e) => return Err(Error::Utf8Decode(e)),
        };

        // create some JSON object from the string
        match serde_json::from_str(&buffer) {
            Ok(v) => Ok(v),
            Err(e) => Err(Error::JsonLoad(e)),
        }
    }
}

/// Brightness can be applied in any context, values from 10 to 100
#[derive(Default, Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct Brightness {
    #[schema(minimum = 10, maximum = 100)]
    value: u8,
}

impl Brightness {
    /// Create a new Brightness with the default value
    ///
    /// # Examples
    ///
    /// ```
    /// use riz::models::Brightness;
    ///
    /// let brightness = Brightness::new();
    /// assert_eq!(brightness.value(), 100);
    /// ```
    pub fn new() -> Self {
        Brightness { value: 100 }
    }

    /// Accessor for our read-only value
    pub fn value(&self) -> u8 {
        self.value
    }

    /// Create a new Brightness value with the given value
    ///
    /// # Returns
    ///   [Option] of [Brightness] when value is within the valid range
    ///
    /// # Examples
    ///
    /// ```
    /// use riz::models::Brightness;
    ///
    /// assert!(Brightness::create(9).is_none());
    /// assert!(Brightness::create(10).is_some());
    /// assert!(Brightness::create(100).is_some());
    /// assert!(Brightness::create(101).is_none());
    /// ```
    ///
    pub fn create(value: u8) -> Option<Self> {
        if Self::valid(value) {
            Some(Brightness { value })
        } else {
            None
        }
    }

    /// Create a new Brightness value with the given value or the
    /// default if the value is not within the valid range
    ///
    /// # Examples
    ///
    /// ```
    /// use riz::models::Brightness;
    ///
    /// assert_eq!(Brightness::create_or(9).value(), 100);
    /// assert_eq!(Brightness::create_or(10).value(), 10);
    /// assert_eq!(Brightness::create_or(100).value(), 100);
    /// assert_eq!(Brightness::create_or(101).value(), 100);
    /// ```
    ///
    pub fn create_or(value: u8) -> Self {
        Brightness {
            value: if Self::valid(value) { value } else { 100 },
        }
    }

    /// Check if the value is within the valid range
    fn valid(value: u8) -> bool {
        (10..=100).contains(&value)
    }
}

/// Speed can be applied to select scenes only, values from 20 to 200
#[derive(Default, Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct Speed {
    #[schema(minimum = 20, maximum = 200)]
    value: u8,
}

impl Speed {
    /// Create a new speed setting with the default value
    ///
    /// # Examples
    ///
    /// ```
    /// use riz::models::Speed;
    ///
    /// assert_eq!(Speed::new().value(), 100);
    /// ```
    ///
    pub fn new() -> Self {
        Speed { value: 100 }
    }

    /// Accessor for our read-only value
    pub fn value(&self) -> u8 {
        self.value
    }

    /// Create a new speed setting with the given value
    ///
    /// # Returns
    ///   [Speed] when value is within the valid range
    ///
    /// # Examples
    ///
    /// ```
    /// use riz::models::Speed;
    ///
    /// assert!(Speed::create(19).is_none());
    /// assert!(Speed::create(20).is_some());
    /// assert!(Speed::create(200).is_some());
    /// assert!(Speed::create(201).is_none());
    /// ```
    ///
    pub fn create(value: u8) -> Option<Self> {
        if Self::valid(value) {
            Some(Speed { value })
        } else {
            None
        }
    }

    /// Create a new speed setting with the given value if within
    /// the valid range, otherwise the default value
    ///
    /// # Examples
    ///
    /// ```
    /// use riz::models::Speed;
    ///
    /// assert_eq!(Speed::create_or(19).value(), 100);
    /// assert_eq!(Speed::create_or(20).value(), 20);
    /// assert_eq!(Speed::create_or(200).value(), 200);
    /// assert_eq!(Speed::create_or(201).value(), 100);
    /// ```
    ///
    pub fn create_or(value: u8) -> Self {
        Speed {
            value: if Self::valid(value) { value } else { 100 },
        }
    }

    fn valid(value: u8) -> bool {
        (20..=200).contains(&value)
    }
}

/// Kelvin sets a temperature mode, values from 1000 to 8000
#[derive(Default, Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct Kelvin {
    #[schema(minimum = 1000, maximum = 8000)]
    kelvin: u16,
}

impl Kelvin {
    /// Create a new Kelvin setting with the default value
    ///
    /// # Examples
    ///
    /// ```
    /// use riz::models::Kelvin;
    ///
    /// assert_eq!(Kelvin::new().kelvin(), 1000);
    /// ```
    ///
    pub fn new() -> Self {
        Kelvin { kelvin: 1000 }
    }

    /// Accessor for our read-only kelvin setting
    pub fn kelvin(&self) -> u16 {
        self.kelvin
    }

    /// Create a new Kelvin setting with the given value
    ///
    /// # Returns
    ///   [Kelvin] when value is within the valid range
    ///
    /// # Examples
    ///
    /// ```
    /// use riz::models::Kelvin;
    ///
    /// assert!(Kelvin::create(999).is_none());
    /// assert!(Kelvin::create(1000).is_some());
    /// assert!(Kelvin::create(8000).is_some());
    /// assert!(Kelvin::create(8001).is_none());
    /// ```
    ///
    pub fn create(kelvin: u16) -> Option<Self> {
        if (1000..=8000).contains(&kelvin) {
            Some(Kelvin { kelvin })
        } else {
            None
        }
    }
}

/// White describes a cool or warm white mode, values from 1 to 100
#[derive(Default, Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct White {
    #[schema(minimum = 1, maximum = 100)]
    value: u8,
}

impl White {
    /// Create a new white setting with the default value
    pub fn new() -> Self {
        White { value: 100 }
    }

    /// Create a new white setting with the given value
    ///
    /// # Returns
    ///   [White] if the value provided is within the valid range
    ///
    /// # Examples
    ///
    /// ```
    /// use riz::models::White;
    ///
    /// assert!(White::create(0).is_none());
    /// assert!(White::create(1).is_some());
    /// assert!(White::create(100).is_some());
    /// assert!(White::create(101).is_none());
    /// ```
    ///
    pub fn create(value: u8) -> Option<Self> {
        if (1..=100).contains(&value) {
            Some(White { value })
        } else {
            None
        }
    }
}

/// Color is any RGB color, values from 0 to 255
#[derive(Default, Debug, Serialize, Deserialize, Clone, ToSchema, PartialEq)]
pub struct Color {
    #[schema(maximum = 255)]
    red: u8,
    #[schema(maximum = 255)]
    green: u8,
    #[schema(maximum = 255)]
    blue: u8,
}

impl Color {
    /// Create a new default color
    ///
    /// # Examples
    ///
    /// ```
    /// use std::str::FromStr;
    /// use riz::models::Color;
    ///
    /// assert_eq!(Color::new(), Color::from_str("0,0,0").unwrap());
    /// assert_ne!(Color::new(), Color::from_str("0,1,0").unwrap());
    /// ```
    ///
    pub fn new() -> Self {
        Color {
            red: 0,
            green: 0,
            blue: 0,
        }
    }

    /// Accessor for this color's read-only red value
    pub fn red(&self) -> u8 {
        self.red
    }

    /// Accessor for this color's read-only green value
    pub fn green(&self) -> u8 {
        self.green
    }

    /// Accessor for this color's read-only blue value
    pub fn blue(&self) -> u8 {
        self.blue
    }
}

impl FromStr for Color {
    type Err = String;

    /// Create a new Color from a string slice
    ///
    /// Expected format is r,g,b where each value can be 0-255,
    /// values outside this range will be converted to zero.
    ///
    /// Examples:
    ///
    /// ```
    /// use std::str::FromStr;
    /// use riz::models::Color;
    ///
    /// assert!(Color::from_str("100,80,240").is_ok());
    /// assert!(Color::from_str("100,80,240,255").is_err());
    /// assert!(Color::from_str("#ffeeff").is_err());
    ///
    /// assert_eq!(
    ///   Color::from_str("1000,-2,256").unwrap(),
    ///   Color::from_str("0,0,0").unwrap()
    /// );
    /// ```
    ///
    fn from_str(s: &str) -> StdResult<Self, String> {
        let parts: Vec<_> = s.split(',').map(|c| c.parse::<u8>().unwrap_or(0)).collect();

        if parts.len() == 3 {
            Ok(Color {
                red: parts[0],
                green: parts[1],
                blue: parts[2],
            })
        } else {
            Err("Invalid color string".to_string())
        }
    }
}

/// API request for a lighting settings change on a [Light]
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct LightRequest {
    // brightness percent, valid from 10 to 100
    // to be used with setbrightness --dim <value>
    brightness: Option<Brightness>,

    // set the rgb color value, valid from 0 to 255
    // to be used with setrgbcolor --r <r> --g <g> --b <b>
    color: Option<Color>,

    // Color changing speed, from 20 to 200 (time %)
    // to be used with setspeed --speed <value>
    speed: Option<Speed>,

    // Color temperature, in kelvins from 1000 to 8000
    // to be used with setcolortemp --temp <value>
    temp: Option<Kelvin>,

    // Scene to select, from enum
    // to be used with setscene --scene <value>
    scene: Option<SceneMode>,

    // If we would like to adjust the light's power
    power: Option<PowerMode>,

    // If we'd like to set the cool white value
    cool: Option<White>,

    // If we'd like to set the warm white value
    warm: Option<White>,
}

impl LightRequest {
    /// Accessor to get this request's optional [PowerMode] setting
    pub fn power(&self) -> Option<&PowerMode> {
        self.power.as_ref()
    }
}

/// Describes a potential emitting state of a [Light]
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub enum PowerMode {
    /// Send a reboot command to the light
    Reboot,

    /// Tell the bulb to emit light
    On,

    /// Tell the bulb to stop emitting light
    Off,
}

/// Preset lighting modes
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema, EnumIter, PartialEq)]
pub enum SceneMode {
    Ocean = 1,
    Romance = 2,
    Sunset = 3,
    Party = 4,
    Fireplace = 5,
    Cozy = 6,
    Forest = 7,
    PastelColors = 8,
    WakeUp = 9,
    Bedtime = 10,
    WarmWhite = 11,
    Daylight = 12,
    CoolWhite = 13,
    NightLight = 14,
    Focus = 15,
    Relax = 16,
    TrueColors = 17,
    TvTime = 18,
    Plantgrowth = 19,
    Spring = 20,
    Summer = 21,
    Fall = 22,
    Deepdive = 23,
    Jungle = 24,
    Mojito = 25,
    Club = 26,
    Christmas = 27,
    Halloween = 28,
    Candlelight = 29,
    GoldenWhite = 30,
    Pulse = 31,
    Steampunk = 32,
    Diwali = 33,
}

impl SceneMode {
    pub fn create(value: u8) -> Option<Self> {
        // this is suboptimal...
        SceneMode::iter().find(|scene| scene.clone() as u8 == value)
    }
}

/// The last context set on the light that the API is aware of.
///
/// This could potentially still be wrong, the API is not the only
/// way to change state on the bulbs, and we don't monitor/poll...
///
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema, PartialEq)]
pub enum LastSet {
    /// The last set context was an RGB color
    Color,

    /// The last set context was a SceneMode
    Scene,

    /// The last set context was a Kelvin temperature
    Temp,

    /// The last set context was a cool white value
    Cool,

    /// The last set context was a warm white value
    Warm,
}

impl LastSet {
    fn from(value: &Payload) -> Option<Self> {
        if value.scene.is_some() {
            return Some(LastSet::Scene);
        }
        if value.get_color().is_some() {
            return Some(LastSet::Color);
        }
        if value.temp.is_some() {
            return Some(LastSet::Temp);
        }
        if value.cool.is_some() {
            return Some(LastSet::Cool);
        }
        if value.warm.is_some() {
            return Some(LastSet::Warm);
        }
        None
    }
}

/// Tracks the last known settings set by Riz, along with the last context
///
/// When new settings are set, old settings that arn't overwritten are
/// left as they were. This allows the UI to set previously set values
/// for all potential contexts, while also displaying the active context.
///
#[serde_with::skip_serializing_none]
#[derive(Debug, Serialize, Deserialize, Clone, ToSchema)]
pub struct LightStatus {
    /// Current color, if set
    color: Option<Color>,

    /// Brightness percentage, if known
    brightness: Option<Brightness>,

    /// If the bulb is emitting light
    emitting: bool,

    /// Currently playing scene, if any
    scene: Option<SceneMode>,

    /// Last set speed value, if known
    speed: Option<Speed>,

    /// Last set light temperature, if known
    temp: Option<Kelvin>,

    /// Cool white value, if known
    cool: Option<White>,

    /// Warm white value, if known
    warm: Option<White>,

    /// Last set value, if any
    last: Option<LastSet>,
}

impl LightStatus {
    /// Accessor to get the last set context by reference
    pub fn last(&self) -> Option<&LastSet> {
        self.last.as_ref()
    }

    /// Accessor to get the last set color by reference
    pub fn color(&self) -> Option<&Color> {
        self.color.as_ref()
    }

    /// Accessor to get the last set brightness value by reference
    pub fn brightness(&self) -> Option<&Brightness> {
        self.brightness.as_ref()
    }

    /// Accessor to get the last known light emitting state
    pub fn emitting(&self) -> bool {
        self.emitting
    }

    /// Accessor to get the last set scene by reference
    pub fn scene(&self) -> Option<&SceneMode> {
        self.scene.as_ref()
    }

    /// Accessor to get the last set speed value by reference
    pub fn speed(&self) -> Option<&Speed> {
        self.speed.as_ref()
    }

    /// Accessor to get the last set temp value by reference
    pub fn temp(&self) -> Option<&Kelvin> {
        self.temp.as_ref()
    }

    /// Accessor to get the last set cool white value by reference
    pub fn cool(&self) -> Option<&White> {
        self.cool.as_ref()
    }

    /// Accessor to get the last set warm white value by reference
    pub fn warm(&self) -> Option<&White> {
        self.warm.as_ref()
    }

    /// Update this status with the values from the other
    ///
    /// Any values set in other become set in self, otherwise
    /// values in self are left untouched.
    ///
    /// Examples:
    ///
    /// ```
    /// use riz::models::{LightStatus, Payload, Speed, Kelvin};
    ///
    /// let mut status = LightStatus::from(&Payload::from(&Kelvin::new()));
    /// assert_eq!(status.temp().unwrap().kelvin(), 1000);
    /// assert!(status.speed().is_none());
    ///
    /// status.update(&LightStatus::from(&Payload::from(&Speed::new())));
    /// assert_eq!(status.temp().unwrap().kelvin(), 1000);
    /// assert_eq!(status.speed().unwrap().value(), 100);
    /// ```
    ///
    pub fn update(&mut self, other: &Self) {
        if let Some(color) = &other.color {
            self.color = Some(color.clone());
        }
        if let Some(brightness) = &other.brightness {
            self.brightness = Some(brightness.clone());
        }
        self.emitting = other.emitting;
        self.scene = other.scene.clone();
        if let Some(speed) = &other.speed {
            self.speed = Some(speed.clone());
        }
        if let Some(temp) = &other.temp {
            self.temp = Some(temp.clone());
        }
        if let Some(cool) = &other.cool {
            self.cool = Some(cool.clone());
        }
        if let Some(warm) = &other.warm {
            self.warm = Some(warm.clone());
        }
        if let Some(last) = &other.last {
            self.last = Some(last.clone());
        }
    }

    fn update_from_payload(&mut self, payload: &Payload) {
        if let Some(color) = payload.get_color() {
            self.color = Some(color);
            self.last = Some(LastSet::Color);
        }
        if let Some(dimming) = payload.dimming {
            self.brightness = Brightness::create(dimming);
        }
        if let Some(speed) = payload.speed {
            self.speed = Speed::create(speed);
        }
        if let Some(temp) = payload.temp {
            self.temp = Kelvin::create(temp);
            self.last = Some(LastSet::Temp);
        }
        if let Some(scene) = payload.scene {
            self.scene = SceneMode::create(scene);
            self.last = Some(LastSet::Scene);
        }
        if let Some(cool) = payload.cool {
            self.cool = White::create(cool);
            self.last = Some(LastSet::Cool);
        }
        if let Some(warm) = payload.warm {
            self.warm = White::create(warm);
            self.last = Some(LastSet::Warm);
        }
    }

    fn update_from_power(&mut self, power: &PowerMode) {
        match power {
            PowerMode::Off => self.emitting = false,
            _ => self.emitting = true,
        }
    }
}

impl From<&Payload> for LightStatus {
    fn from(payload: &Payload) -> Self {
        let color = payload.get_color();

        let brightness = if let Some(value) = payload.dimming {
            Brightness::create(value)
        } else {
            None
        };

        let scene = if let Some(scene) = payload.scene {
            SceneMode::create(scene)
        } else {
            None
        };

        let speed = if let Some(speed) = payload.speed {
            Speed::create(speed)
        } else {
            None
        };

        let temp = if let Some(temp) = payload.temp {
            Kelvin::create(temp)
        } else {
            None
        };

        let cool = if let Some(cool) = payload.cool {
            White::create(cool)
        } else {
            None
        };

        let warm = if let Some(warm) = payload.warm {
            White::create(warm)
        } else {
            None
        };

        LightStatus {
            color,
            brightness,
            emitting: true, // we don't actually know this here...
            scene,
            speed,
            temp,
            cool,
            warm,
            last: LastSet::from(payload),
        }
    }
}

impl From<&PowerMode> for LightStatus {
    fn from(power: &PowerMode) -> Self {
        LightStatus {
            color: None,
            brightness: None,
            emitting: !matches!(power, PowerMode::Off),
            scene: None,
            speed: None,
            temp: None,
            cool: None,
            warm: None,
            last: None,
        }
    }
}

impl From<&BulbStatus> for LightStatus {
    fn from(bulb: &BulbStatus) -> Self {
        let res = &bulb.result;

        LightStatus {
            color: res.get_color(),
            brightness: Brightness::create(res.dimming.unwrap_or(0)),
            cool: White::create(res.cool.unwrap_or(0)),
            warm: White::create(res.warm.unwrap_or(0)),
            emitting: res.emitting,
            scene: SceneMode::create(res.scene),
            // NB: these are not returned from getPilot...
            //     best we can do is track what we set then
            speed: None,
            temp: None,
            last: None,
        }
    }
}

/// Bulb status, as reported by the bulb.
///
/// Several lighting settings are available as settings, but we can't
/// get the state back out of the bulb.
///
/// BulbStatus is *only* what the bulb reports, it is then merged into a
/// [LightStatus] which adds the logic to track settings the bulb will
/// accept but not report.
#[derive(Debug, Serialize, Deserialize, Clone)]
struct BulbStatus {
    env: String,
    method: String,
    result: BulbStatusResult,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct BulbStatusResult {
    /// red (0-255)
    #[serde(rename = "r")]
    red: Option<u8>,

    /// green (0-255)
    #[serde(rename = "g")]
    green: Option<u8>,

    /// blue (0-255)
    #[serde(rename = "b")]
    blue: Option<u8>,

    /// dimming percent (0-100)
    dimming: Option<u8>,

    /// bulb wifi mac address
    mac: String,

    /// true when bulb state is on
    #[serde(rename = "state")]
    emitting: bool,

    /// current scene ID, zero if not playing a scene
    #[serde(rename = "sceneId")]
    scene: u8,

    /// bulb's wifi signal strength
    rssi: i32,

    /// bulb's cool white value
    #[serde(rename = "c")]
    cool: Option<u8>,

    /// bulb's warm white value
    #[serde(rename = "w")]
    warm: Option<u8>,
}

impl BulbStatusResult {
    fn get_color(&self) -> Option<Color> {
        if let (Some(red), Some(green), Some(blue)) = (self.red, self.green, self.blue) {
            Some(Color { red, green, blue })
        } else {
            None
        }
    }
}

/// Response which could alter the state of a [Light]
///
/// Used with [Light::process_reply] or [Room::process_reply]. Or use
/// [crate::Storage::process_reply] to also update the `rooms.json`
///
#[derive(Debug)]
pub struct LightingResponse {
    ip: Ipv4Addr,
    response: LightingResponseType,
}

impl LightingResponse {
    /// Create a [LightingResponse] for a [Ipv4Addr] from a [Payload]
    pub fn payload(ip: Ipv4Addr, payload: Payload) -> Self {
        LightingResponse {
            ip,
            response: LightingResponseType::Payload(payload),
        }
    }

    /// Create a [LightingResponse] for a [Ipv4Addr] from a [PowerMode]
    pub fn power(ip: Ipv4Addr, power: PowerMode) -> Self {
        LightingResponse {
            ip,
            response: LightingResponseType::Power(power),
        }
    }

    /// Create a [LightingResponse] for a [Ipv4Addr] from a [LightStatus]
    pub fn status(ip: Ipv4Addr, status: LightStatus) -> Self {
        LightingResponse {
            ip,
            response: LightingResponseType::Status(status),
        }
    }
}

/// Reply path payload details for modifying [Light] state
#[derive(Debug)]
pub enum LightingResponseType {
    /// Response from any lighting setting change
    Payload(Payload),

    /// Response from any power (emitting) setting change
    Power(PowerMode),

    /// Response from a bulb status fetch
    Status(LightStatus),
}

/// JSON payload to send at Wiz lights to modify their settings
///
/// You can create a singular payload by using one of the [From] trait
/// implementations. Or create a new empty payload and add attributes to
/// it with the helper methods.
///
#[serde_with::skip_serializing_none]
#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct Payload {
    #[serde(rename = "sceneId")]
    scene: Option<u8>,

    dimming: Option<u8>,
    speed: Option<u8>,
    temp: Option<u16>,

    #[serde(rename = "r")]
    red: Option<u8>,
    #[serde(rename = "g")]
    green: Option<u8>,
    #[serde(rename = "b")]
    blue: Option<u8>,

    #[serde(rename = "c")]
    cool: Option<u8>,
    #[serde(rename = "w")]
    warm: Option<u8>,
}

impl Payload {
    /// Create a new blank payload
    ///
    /// Note that at least one helper method must be called if creating a
    /// payload this way, or the payload will be invalid and cause an error
    /// if you try to use it with a [Light::set] call.
    ///
    /// You can stack as many modes in a single call as you want. The light
    /// will determine if it can set that combination of settings. And if it
    /// can't, will make a best effort to set something close.
    ///
    /// # Examples
    ///
    /// ```
    /// use riz::models::Payload;
    ///
    /// let mut payload = Payload::new();
    /// assert_eq!(payload.is_valid(), false);
    /// ```
    ///
    pub fn new() -> Self {
        Payload {
            scene: None,
            dimming: None,
            speed: None,
            temp: None,
            red: None,
            green: None,
            blue: None,
            cool: None,
            warm: None,
        }
    }

    /// Checks if this payload is valid
    ///
    /// Note that speed is not valid on it's own, it must be set with a
    /// scene mode as well (Wiz limitation).
    ///
    /// # Examples
    ///
    /// ```
    /// use riz::models::{Payload, SceneMode, Speed};
    ///
    /// let mut payload = Payload::new();
    ///
    /// payload.speed(&Speed::create(100).unwrap());
    /// assert_eq!(payload.is_valid(), false);
    ///
    /// payload.scene(&SceneMode::Focus);
    /// assert_eq!(payload.is_valid(), true);
    /// ```
    ///
    pub fn is_valid(&self) -> bool {
        self.scene.is_some()
            || self.dimming.is_some()
            || self.temp.is_some()
            || (self.red.is_some() && self.green.is_some() && self.blue.is_some())
            || self.cool.is_some()
            || self.warm.is_some()
    }

    /// Set the SceneMode to use in this payload, by reference
    ///
    /// # Examples
    ///
    /// ```
    /// use riz::models::{Payload, SceneMode};
    ///
    /// let mut payload = Payload::new();
    /// payload.scene(&SceneMode::Focus);
    /// assert_eq!(payload.is_valid(), true);
    /// ```
    ///
    pub fn scene(&mut self, scene: &SceneMode) {
        self.scene = Some(scene.clone() as u8);
    }

    /// Set the Brightness value in this payload.
    ///
    /// Note that brightness can be applied to any context,
    /// as long as the bulb is emitting.
    ///
    /// # Examples
    ///
    /// ```
    /// use riz::models::{Payload, Brightness};
    ///
    /// let mut payload = Payload::new();
    /// payload.brightness(&Brightness::create(100).unwrap());
    /// assert_eq!(payload.is_valid(), true);
    /// ```
    ///
    pub fn brightness(&mut self, brightness: &Brightness) {
        self.dimming = Some(brightness.value);
    }

    /// Set the speed value in this payload, by reference
    ///
    /// Speed is only relevant when also setting a SceneMode.
    /// If speed is sent with other attributes and not a scene,
    /// the other attributes will set the context on the bulb.
    /// However, if you also use the payload to update state,
    /// the speed value will still be reflected in the light's
    /// last known status.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    /// use std::str::FromStr;
    /// use riz::models::{Light, Payload, LastSet, Color, Speed, LightingResponse};
    ///
    /// let ip = Ipv4Addr::from_str("10.1.2.3").unwrap();
    /// let mut light = Light::new(ip, None);
    ///
    /// let mut payload = Payload::new();
    /// payload.speed(&Speed::create(100).unwrap());
    /// payload.color(&Color::from_str("0,0,255").unwrap());
    ///
    /// let resp = LightingResponse::payload(ip, payload);
    /// assert!(light.process_reply(&resp));
    ///
    /// let status = light.status().unwrap();
    /// assert_eq!(status.last().unwrap(), &LastSet::Color);
    /// assert_eq!(status.speed().unwrap().value(), 100);
    /// ```
    ///
    pub fn speed(&mut self, speed: &Speed) {
        self.speed = Some(speed.value);
    }

    /// Set the temperature value in this payload, by reference
    ///
    /// Note that it is not possible to retrieve this temperature value
    /// back from the bulb itself. Last known settings for this value are
    /// from storing the state after each set call only.
    ///
    /// # Examples
    ///
    /// ```
    /// use riz::models::{Payload, Kelvin};
    ///
    /// let mut payload = Payload::new();
    /// payload.temp(&Kelvin::create(4000).unwrap());
    /// assert_eq!(payload.is_valid(), true);
    /// ```
    ///
    pub fn temp(&mut self, temp: &Kelvin) {
        self.temp = Some(temp.kelvin);
    }

    /// Set the RGB color mode in this payload, by reference
    ///
    /// # Examples
    ///
    /// ```
    /// use std::str::FromStr;
    /// use riz::models::{Payload, Color};
    ///
    /// let mut payload = Payload::new();
    /// payload.color(&Color::from_str("255,255,255").unwrap());
    /// assert_eq!(payload.is_valid(), true);
    /// ```
    ///
    pub fn color(&mut self, color: &Color) {
        self.red = Some(color.red);
        self.green = Some(color.green);
        self.blue = Some(color.blue);
    }

    /// Set the cool white value in this payload, by reference
    ///
    /// This can be used on it's own, some scenes might also use it
    ///
    /// # Examples
    ///
    /// ```
    /// use riz::models::{Payload, White};
    ///
    /// let mut payload = Payload::new();
    /// payload.cool(&White::create(50).unwrap());
    /// assert_eq!(payload.is_valid(), true);
    /// ```
    ///
    pub fn cool(&mut self, cool: &White) {
        self.cool = Some(cool.value);
    }

    /// Set the warm white value in this payload, by reference
    ///
    /// This can be used on it's own, some scenes might also use it
    ///
    /// # Examples
    ///
    /// ```
    /// use riz::models::{Payload, White};
    ///
    /// let mut payload = Payload::new();
    /// payload.warm(&White::create(50).unwrap());
    /// assert_eq!(payload.is_valid(), true);
    /// ```
    ///
    pub fn warm(&mut self, warm: &White) {
        self.warm = Some(warm.value);
    }

    /// Helper method to create a color when we have one set
    fn get_color(&self) -> Option<Color> {
        if let (Some(red), Some(green), Some(blue)) = (self.red, self.green, self.blue) {
            Some(Color { red, green, blue })
        } else {
            None
        }
    }
}

impl From<&SceneMode> for Payload {
    fn from(scene: &SceneMode) -> Self {
        let mut p = Payload::new();
        p.scene(scene);
        p
    }
}

impl From<&Kelvin> for Payload {
    fn from(kelvin: &Kelvin) -> Self {
        let mut p = Payload::new();
        p.temp(kelvin);
        p
    }
}

impl From<&Color> for Payload {
    fn from(color: &Color) -> Self {
        let mut p = Payload::new();
        p.color(color);
        p
    }
}

impl From<&Speed> for Payload {
    fn from(speed: &Speed) -> Self {
        let mut p = Payload::new();
        p.speed(speed);
        p
    }
}

impl From<&LightRequest> for Payload {
    fn from(req: &LightRequest) -> Self {
        let mut p = Payload::new();
        if let Some(brightness) = &req.brightness {
            p.brightness(brightness);
        }
        if let Some(color) = &req.color {
            p.color(color);
        }
        if let Some(speed) = &req.speed {
            p.speed(speed);
        }
        if let Some(temp) = &req.temp {
            p.temp(temp);
        }
        if let Some(scene) = &req.scene {
            p.scene(scene);
        }
        if let Some(cool) = &req.cool {
            p.cool(cool);
        }
        if let Some(warm) = &req.warm {
            p.warm(warm);
        }
        p
    }
}

impl From<&Brightness> for Payload {
    fn from(brightness: &Brightness) -> Self {
        let mut p = Payload::new();
        p.brightness(brightness);
        p
    }
}
