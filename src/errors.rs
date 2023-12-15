use std::sync::mpsc::SendError;
use std::{net::Ipv4Addr, string::FromUtf8Error};

use crate::worker::{DispatchMessage, ReplyMessage};
use uuid::Uuid;

/// All potential errors in riz
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Used when failing to dump json
    #[error("failed to dump json: {0:?}")]
    JsonDump(serde_json::Error),

    /// Used when failing to load json
    #[error("failed to load json: {0:?}")]
    JsonLoad(serde_json::Error),

    /// Some socket error when communicating with a bulb
    #[error("socket {action} error: {err:?}")]
    Socket { action: String, err: std::io::Error },

    /// Failed to decode UDP response bytes as UTF-8
    #[error("utf8 decoding error: {0:?}")]
    Utf8Decode(FromUtf8Error),

    /// Used when trying to set a [crate::models::Payload] with no attributes
    #[error("invalid payload; no attributes set")]
    NoAttribute,

    /// Attempting to look up or modify a room which doesn't exist
    #[error("room not found {0}")]
    RoomNotFound(Uuid),

    /// Attempting to look up or modify a light which doesn't exist
    #[error("light {light_id:?} not found in room {room_id:?}")]
    LightNotFound { room_id: Uuid, light_id: Uuid },

    /// Attempting to add a light with an invalid IP
    #[error("light with ip {ip} is invalid because the IP is {reason}")]
    InvalidIP { ip: Ipv4Addr, reason: String },

    /// When modifying the room's details results in no change
    #[error("no change for room {0}")]
    NoChangeRoom(Uuid),

    /// When modifying the light's details results in no change
    #[error("no change for light {light_id:?} in room {room_id:?}")]
    NoChangeLight { room_id: Uuid, light_id: Uuid },

    /// When modifying a light in a room without lights
    #[error("no lights in room {0}")]
    NoLights(Uuid),

    /// Unable to parse a [crate::models::Color] from the given [String]
    #[error("invalid color string: {0}")]
    InvalidColorString(String),

    /// Unable to queue work, broken channel maybe
    #[error("unable to queue work: {0:?}")]
    Dispatch(SendError<DispatchMessage>),

    /// Unable to process return path from worker
    #[error("unable to process work: {0:?}")]
    Reply(SendError<ReplyMessage>),
}

impl Error {
    /// Create a new socket error
    pub fn socket(action: &str, err: std::io::Error) -> Self {
        Error::Socket {
            action: action.to_string(),
            err,
        }
    }

    /// Create a new light not found error
    pub fn light_not_found(room_id: &Uuid, light_id: &Uuid) -> Self {
        Error::LightNotFound {
            room_id: *room_id,
            light_id: *light_id,
        }
    }

    /// Create a new invalid IP error
    pub fn invalid_ip(ip: &Ipv4Addr, reason: &str) -> Self {
        Error::InvalidIP {
            ip: *ip,
            reason: reason.to_string(),
        }
    }

    /// Create a new no change light error
    pub fn no_change_light(room_id: &Uuid, light_id: &Uuid) -> Self {
        Error::NoChangeLight {
            room_id: *room_id,
            light_id: *light_id,
        }
    }
}

/// Hacky implementation of PartialEq for testing
#[cfg(test)]
impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        self.to_string() == other.to_string()
    }
}
