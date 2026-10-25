use super::{NOT_A_COLLECTION_LINK, Settings, preview_collection};
use crate::config::Config;
use crate::osu_db::OsuClient;
use std::path::PathBuf;

#[test]
fn settings_round_trip_through_the_shared_config() {
    let settings = Settings {
        client: OsuClient::Stable,
        osu_path: Some(PathBuf::from("/games/osu!")),
        download_dir: PathBuf::from("/maps"),
    };
    let mut config = Config::default();

    settings.apply_to(&mut config);

    assert_eq!(config.recent.osu_client, Some(OsuClient::Stable));
    assert_eq!(config.recent.osu_path.as_deref(), Some("/games/osu!"));
    assert_eq!(config.recent.directory.as_deref(), Some("/maps"));
    assert_eq!(Settings::from_config(&config), settings);
}

#[test]
fn empty_config_defaults_to_lazer_autodetect_and_a_download_folder() {
    let settings = Settings::from_config(&Config::default());

    assert_eq!(settings.client, OsuClient::Lazer);
    assert_eq!(settings.osu_path, None);
    assert!(settings.download_dir.ends_with("osu-collect"));
}

#[test]
fn tilde_in_the_stored_directory_is_expanded() {
    let mut config = Config::default();
    config.recent.directory = Some("~/maps".to_string());

    let settings = Settings::from_config(&config);

    assert!(!settings.download_dir.starts_with("~"));
    assert!(settings.download_dir.ends_with("maps"));
}

#[tokio::test]
async fn preview_rejects_input_that_is_not_a_collection_link() {
    let err = preview_collection("definitely not a link")
        .await
        .unwrap_err();
    assert_eq!(err.to_string(), NOT_A_COLLECTION_LINK);
}
