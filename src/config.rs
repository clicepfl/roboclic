use envconfig::Envconfig;
use serde::Deserialize;
use std::{collections::HashMap, fs::File, io::Read, sync::OnceLock};

#[derive(Deserialize)]
struct JsonConfig {
    committee: Vec<String>,
    access_control: HashMap<String, Vec<i64>>,
}

#[derive(Envconfig)]
pub struct EnvConfig {
    #[envconfig(from = "BOT_TOKEN")]
    pub bot_token: String,
    #[envconfig(from = "CONFIG_FILE", default = "config.json")]
    pub config_file: String,
}

pub struct Config {
    pub committee: Vec<String>,
    pub bot_token: String,
    pub access_control: HashMap<String, Vec<i64>>,
}

static CONFIG: OnceLock<Config> = OnceLock::new();
pub fn config() -> &'static Config {
    CONFIG.get_or_init(|| {
        let env_config = EnvConfig::init_from_env().unwrap();

        let mut json_config = String::new();
        File::open(&env_config.config_file)
            .unwrap_or_else(|_| panic!("Could not open config file at {}", env_config.config_file))
            .read_to_string(&mut json_config)
            .unwrap_or_else(|_| {
                panic!(
                    "Could not read from config file at {}",
                    env_config.config_file
                )
            });

        let json_config: JsonConfig =
            serde_json::from_str(json_config.as_str()).expect("Could not parse config file");

        Config {
            committee: json_config.committee,
            bot_token: env_config.bot_token,
            access_control: json_config.access_control,
        }
    })
}
