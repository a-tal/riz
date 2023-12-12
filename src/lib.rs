//! Riz - Wiz Light Control Library
//!
//! # Examples
//!
//! ```
//! use std::net::Ipv4Addr;
//! use std::str::FromStr;
//! use riz::models::{Light, Payload, LightingResponse, Color, LastSet, SceneMode};
//!
//! let light = Light::new(Ipv4Addr::from_str("192.168.1.91").unwrap(), None);
//!
//! // set does not require mutability
//! let resp = light.set(&Payload::from(&Color::from_str("255,0,0").unwrap())).unwrap();
//!
//! // updating the light's status (and rooms.json) requires mutability
//! let mut light = light;
//! assert!(light.process_reply(&resp));
//!
//! // after we have called .process_reply() the light's status is correct
//! let status = light.status().unwrap();
//! assert_eq!(status.last().unwrap(), &LastSet::Color);
//!
//! // status will have all the last set values it knows about
//! let color = status.color().unwrap();
//! assert_eq!(color.red(), 255);
//! assert_eq!(color.green(), 0);
//! assert_eq!(color.blue(), 0);
//!
//! // for example, if we switch to a scene now, the color
//! // values will remain. use status.last to determine context
//! let resp = light.set(&Payload::from(&SceneMode::Focus)).unwrap();
//! assert!(light.process_reply(&resp));
//!
//! let status = light.status().unwrap();
//! assert_eq!(status.last().unwrap(), &LastSet::Scene);
//!
//! assert_eq!(status.scene().unwrap(), &SceneMode::Focus);
//!
//! let color = status.color().unwrap();
//! assert_eq!(color.red(), 255);
//! assert_eq!(color.green(), 0);
//! assert_eq!(color.blue(), 0);
//! ```
//!
//! # API
//!
//! Note that all Riz API routes are also documented in OpenAPI spec.
//!
//! You can view the OpenAPI [locally](http://localhost:8080/v1/swagger-ui/)
//! if you have Riz API running.
//!
//! For an example of UI integration with this API, check out
//! [Riz-UI](https://github.com/a-tal/riz-ui)
//!
//! # CLI
//!
//! You can modify lights directly through the CLI. State will not be
//! updated (`rooms.json` is only written by the API).
//!
//! ```bash
//! $ riz --help
//! Riz light control CLI
//!
//! Usage: riz [OPTIONS] [IP]...
//!
//! Arguments:
//!   [IP]...  Bulb IPv4 address(es)
//!
//! Options:
//!   -b, --brightness <BRIGHTNESS>  Set the bulb brightness (10-100)
//!   -c, --color <COLOR>            Set the bulb color as r,g,b (0-255)
//!   -C, --cool <COOL>              Set the cool white value (1-100)
//!   -W, --warm <WARM>              Set the warm white value (1-100)
//!   -p, --speed <SPEED>            Set the bulb speed (20-200)
//!   -t, --temp <TEMP>              Set the bulb temperature in Kelvin (1000-8000)
//!   -l, --list                     List the available scene IDs
//!   -s, --scene <SCENE>            Set the scene by ID
//!   -o, --on                       Turn the bulb on
//!   -f, --off                      Turn the bulb off
//!   -r, --reboot                   Reboot the bulb
//!   -i, --status                   Get the current bulb status
//!   -h, --help                     Print help
//!   -V, --version                  Print version
//! ```
//!

pub mod models;

mod routes;
mod storage;
mod worker;

pub use routes::{health, lights, rooms};
pub use storage::Storage;
pub use worker::Worker;
