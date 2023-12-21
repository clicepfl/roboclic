use envconfig::Envconfig;
use std::sync::OnceLock;

#[derive(Envconfig)]
pub struct Config {
    #[envconfig(from = "BOT_TOKEN")]
    pub bot_token: String,
    #[envconfig(from = "CONFIG_FILE")]
    pub config_file: String,
    #[envconfig(from = "DATA_DIR")]
    pub data_dir: String,
    #[envconfig(from = "DATABASE_URL")]
    pub database_url: Option<String>,
    #[envconfig(from = "ADMIN_TOKEN")]
    pub admin_token: String,
}

static CONFIG: OnceLock<Config> = OnceLock::new();
pub fn config() -> &'static Config {
    CONFIG.get_or_init(|| Config::init_from_env().unwrap())
}
