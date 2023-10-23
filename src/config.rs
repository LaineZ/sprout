use std::net::{IpAddr, Ipv4Addr};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub postgres_url: String,
    pub bind_address: IpAddr,
    pub port: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            postgres_url: String::new(),
            port: 3030,
            bind_address: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        }
    }
}

impl Config {
    pub fn new_from_file() -> Config {
        match std::fs::read_to_string("config.toml") {
            Ok(string) => {
                return toml::from_str(&string).unwrap_or_default();
            }
            Err(_) => {
                return Config::default();
            }
        }
    }

    pub fn save(&self) -> anyhow::Result<()> {
        std::fs::write("config.toml", toml::to_string(self).unwrap())?;
        Ok(())
    }
}
