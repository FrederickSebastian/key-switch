use key_switch_core::{application_directory, APP_IDENTIFIER};
use tauri::Manager;

#[test]
fn tui_directory_matches_tauri_and_window_is_created_explicitly() {
    let mut context = tauri::test::mock_context(tauri::test::noop_assets());
    context.config_mut().identifier = APP_IDENTIFIER.into();
    let app = tauri::test::mock_builder().build(context).unwrap();
    assert_eq!(
        app.path().app_data_dir().unwrap(),
        application_directory().unwrap()
    );
    let config: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    assert_eq!(config["identifier"], APP_IDENTIFIER);
    assert_eq!(config["app"]["windows"][0]["create"], false);
}
