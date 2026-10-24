use key_switch_core::{application_directory, APP_IDENTIFIER};
use std::process::Command;

#[test]
fn path_child() {
    if std::env::var_os("KEY_SWITCH_PATH_TEST").is_none() {
        return;
    }
    let expected = std::env::var_os("KEY_SWITCH_PATH_EXPECTED")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| dirs::data_dir().unwrap().join(APP_IDENTIFIER));
    assert_eq!(application_directory().unwrap(), expected);
}
#[test]
fn uses_native_directory_and_linux_xdg_rules() {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "path_child"])
        .env("KEY_SWITCH_PATH_TEST", "1");
    assert!(command.output().unwrap().status.success());
    #[cfg(target_os = "linux")]
    {
        let directory = tempfile::tempdir().unwrap();
        let data = directory.path().join("中文 用户数据");
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", "path_child"])
            .env("KEY_SWITCH_PATH_TEST", "1")
            .env("XDG_DATA_HOME", &data)
            .env("KEY_SWITCH_PATH_EXPECTED", data.join(APP_IDENTIFIER));
        assert!(command.output().unwrap().status.success());
        command
            .env_remove("XDG_DATA_HOME")
            .env("HOME", directory.path())
            .env(
                "KEY_SWITCH_PATH_EXPECTED",
                directory.path().join(".local/share").join(APP_IDENTIFIER),
            );
        assert!(command.output().unwrap().status.success());
    }
}
