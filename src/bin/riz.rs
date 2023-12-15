use std::{net::Ipv4Addr, str::FromStr};

use clap::Parser;
use convert_case::{Case, Casing};
use riz::{
    models::{
        Brightness, Color, Kelvin, Light, LightingResponse, Payload, PowerMode, SceneMode, Speed,
        White,
    },
    Result,
};
use strum::IntoEnumIterator;

#[derive(Debug, Parser)]
#[command(author, version, about = "Riz light control CLI", long_about = None)]
struct Args {
    /// Bulb IPv4 address(es)
    ip: Option<Vec<Ipv4Addr>>,

    #[arg(short, long)]
    /// Set the bulb brightness (10-100)
    brightness: Option<u8>,

    #[arg(short, long)]
    /// Set the bulb color as r,g,b (0-255)
    color: Option<String>,

    #[arg(short = 'C', long)]
    /// Set the cool white value (1-100)
    cool: Option<u8>,

    #[arg(short = 'W', long)]
    /// Set the warm white value (1-100)
    warm: Option<u8>,

    #[arg(short = 'p', long)]
    /// Set the bulb speed (20-200)
    speed: Option<u8>,

    #[arg(short, long)]
    /// Set the bulb temperature in Kelvin (1000-8000)
    temp: Option<u16>,

    #[arg(short, long)]
    /// List the available scene IDs
    list: bool,

    #[arg(short, long)]
    /// Set the scene by ID
    scene: Option<u8>,

    #[arg(short, long)]
    /// Turn the bulb on
    on: bool,

    #[arg(short = 'f', long)]
    /// Turn the bulb off
    off: bool,

    #[arg(short, long)]
    /// Reboot the bulb
    reboot: bool,

    #[arg(short = 'i', long)]
    /// Get the current bulb status
    status: bool,
}

fn print_scenes() {
    for scene in SceneMode::iter() {
        let s = format!("{:?}", scene);
        println!(
            "{:>6} => {}",
            scene as u8,
            s.from_case(Case::Pascal).to_case(Case::Title)
        );
    }
}

fn print_response(res: Result<LightingResponse>) {
    if let Err(e) = res {
        eprintln!("Error: {:?}", e);
    }
}

fn modify_light(args: &Args, light: Light) {
    if args.status {
        match light.get_status() {
            Ok(status) => println!("{}", serde_json::to_string_pretty(&status).unwrap()),
            Err(e) => eprintln!("Failed to get bulb status: {:?}", e),
        }
        return;
    }

    // only make at most one power action...
    if args.on {
        print_response(light.set_power(&PowerMode::On));
    } else if args.off {
        print_response(light.set_power(&PowerMode::Off));
    } else if args.reboot {
        print_response(light.set_power(&PowerMode::Reboot));
    }

    // we can combine all other actions into one remote command
    // how much sense that makes is context dependant...
    let mut payload = Payload::new();

    if let Some(scene) = args.scene {
        if let Some(scene) = SceneMode::create(scene) {
            payload.scene(&scene);
        } else {
            eprintln!("Invalid scene ID: {}", scene);
        }
    }

    if let Some(brightness) = args.brightness {
        if let Some(brightness) = Brightness::create(brightness) {
            payload.brightness(&brightness);
        } else {
            eprintln!("Invalid brightness value: {}", brightness);
        }
    }

    if let Some(color) = &args.color {
        if let Ok(color) = Color::from_str(color) {
            payload.color(&color);
        } else {
            eprintln!("Invalid color: {}", color);
        }
    }

    if let Some(speed) = args.speed {
        if let Some(speed) = Speed::create(speed) {
            payload.speed(&speed);
        } else {
            eprintln!("Invalid speed value: {}", speed);
        }
    }

    if let Some(temp) = args.temp {
        if let Some(temp) = Kelvin::create(temp) {
            payload.temp(&temp);
        } else {
            eprintln!("Invalid temp value: {}", temp);
        }
    }

    if let Some(cool) = args.cool {
        if let Some(cool) = White::create(cool) {
            payload.cool(&cool);
        } else {
            eprintln!("Invalid cool white value: {}", cool);
        }
    }

    if let Some(warm) = args.warm {
        if let Some(warm) = White::create(warm) {
            payload.warm(&warm);
        } else {
            eprintln!("Invalid warm white value: {}", warm);
        }
    }

    if payload.is_valid() {
        print_response(light.set(&payload));
    }
}

fn main() {
    let args = Args::parse();

    if args.list {
        print_scenes();
        return;
    }

    let ips = match &args.ip {
        Some(ips) => ips,
        None => {
            eprintln!("IP address is required!");
            return;
        }
    };

    for ip in ips {
        modify_light(&args, Light::new(*ip, None));
    }
}
