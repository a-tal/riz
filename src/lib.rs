//! Riz - Wiz Light Control Library
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
