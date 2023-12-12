# Riz

Rust API (and CLI) for controlling Wiz lights.

## UI

Check out [Riz UI](https://github.com/a-tal/riz-ui) for an example of integrating a web UI with this API.

## Dev

Both dev utilities are designed to be quick to iterate with, but `dev.sh` is faster.

### `run.sh`

- Use the separate build & run dockerfiles to start the API with Docker locally

### `dev.sh`

- Start the API without docker (requires Rust installed)

## Config

| Env Var            | Default               | Description                                                    |
| ------------------ | --------------------- | -------------------------------------------------------------- |
| `RIZ_PORT`         | 8080                  | API listening port                                             |
| `RIZ_STORAGE_PATH` | .                     | Path to storage (`rooms.json` must be writable by running UID) |
| `RIZ_CORS_ORIGIN`  | http://localhost:8000 | Allowed CORS origin                                            |

## Docker

| Build Arg | Default | Description |
| --------- | ------- | ----------- |
| `UID`     | 10010   | Running UID |

By default, `RIZ_STORAGE_PATH` is configured to `/data`; which is a VOLUME mount you may use.

The running dockerfiles include a healthcheck configuration.

## CLI

To use the CLI; either pull the binary from the build container, or build this project locally with `cargo build --release`. The CLI will built as `target/release/riz`. Move that into your `$PATH` somewhere if you want to use `riz` anywhere.

```bash
$ riz --help
Riz light control CLI

Usage: riz [OPTIONS] [IP]...

Arguments:
  [IP]...  Bulb IPv4 address(es)

Options:
  -b, --brightness <BRIGHTNESS>  Set the bulb brightness (10-100)
  -c, --color <COLOR>            Set the bulb color as r,g,b (0-255)
  -C, --cool <COOL>              Set the cool white value (1-100)
  -W, --warm <WARM>              Set the warm white value (1-100)
  -p, --speed <SPEED>            Set the bulb speed (20-200)
  -t, --temp <TEMP>              Set the bulb temperature in Kelvin (1000-8000)
  -l, --list                     List the available scene IDs
  -s, --scene <SCENE>            Set the scene by ID
  -o, --on                       Turn the bulb on
  -f, --off                      Turn the bulb off
  -r, --reboot                   Reboot the bulb
  -i, --status                   Get the current bulb status
  -h, --help                     Print help
  -V, --version                  Print version
```
