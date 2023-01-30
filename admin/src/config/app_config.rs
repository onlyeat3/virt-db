use crate::utils;
use actix_settings::BasicSettings;
use log::info;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ApplicationSettings {
    pub jwt_secret: String,
    pub mysql_url: String,
    pub static_dir: String,
}

pub static mut SETTINGS: Option<BasicSettings<ApplicationSettings>> = None;

pub fn load_config() -> BasicSettings<ApplicationSettings> {
    unsafe {
        let mut config_file_location = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        config_file_location.push("actix.toml");
        let config_file_load_err_msg = &*format!(
            "Failed to parse `Settings` from {:?}",
            config_file_location.to_str()
        );
        let settings = BasicSettings::parse_toml(
            config_file_location
                .to_str()
                .expect(config_file_load_err_msg),
        )
        .expect(config_file_load_err_msg);
        info!("settings:{:?}", settings);
        let _ = SETTINGS.insert(settings);
        SETTINGS.clone().unwrap()
    }
}
