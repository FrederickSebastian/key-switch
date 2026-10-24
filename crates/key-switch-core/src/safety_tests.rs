use super::*;
use std::{
    collections::HashMap,
    sync::atomic::{AtomicUsize, Ordering},
};
use tempfile::TempDir;

#[derive(Default)]
pub(super) struct MockCredentials {
    pub(super) values: Mutex<HashMap<String, String>>,
    calls: AtomicUsize,
    fail_at: AtomicUsize,
}
impl MockCredentials {
    fn fault(&self) -> Result<(), CommandError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if call == self.fail_at.load(Ordering::SeqCst) {
            Err("系统密钥库不可用".into())
        } else {
            Ok(())
        }
    }
    fn fail_next(&self, offset: usize) {
        self.fail_at
            .store(self.calls.load(Ordering::SeqCst) + offset, Ordering::SeqCst);
    }
}
impl Credentials for MockCredentials {
    fn get(&self, id: &str) -> Result<String, CommandError> {
        self.fault()?;
        self.values
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or("无法读取系统密钥库".into())
    }
    fn set(&self, id: &str, value: &str) -> Result<(), CommandError> {
        self.fault()?;
        self.values.lock().unwrap().insert(id.into(), value.into());
        Ok(())
    }
    fn delete(&self, id: &str) -> Result<(), CommandError> {
        self.fault()?;
        self.values.lock().unwrap().remove(id);
        Ok(())
    }
}
pub(super) fn fixture() -> (TempDir, Arc<MockCredentials>, Core) {
    let dir = tempfile::tempdir().unwrap();
    let credentials = Arc::new(MockCredentials::default());
    let app = Core::new(
        InstanceLock::acquire(dir.path()).unwrap(),
        credentials.clone(),
    )
    .unwrap();
    create_provider(
        &app,
        CreateProviderInput {
            id: "p".into(),
            name: "用户供应商".into(),
            abbreviation: "P".into(),
            tone: "blue".into(),
            logo: None,
            kind: "custom".into(),
            platform_url: None,
            validation: ValidationConfig::None,
        },
    )
    .unwrap();
    (dir, credentials, app)
}
pub(super) fn add_key(app: &Core) -> ApiKeySummary {
    create_api_key(
        app,
        CreateKeyInput {
            provider_id: "p".into(),
            remark: "用户备注".into(),
            value: "fixture-private-credential".into(),
        },
    )
    .unwrap()
}

#[test]
fn lock_excludes_duplicates_and_survives_leftover_file() {
    let dir = tempfile::tempdir().unwrap();
    let first = InstanceLock::acquire(dir.path()).unwrap();
    assert!(matches!(
        InstanceLock::acquire(dir.path()),
        Err(StartupError::AlreadyRunning)
    ));
    drop(first);
    assert!(dir.path().join("key-switch.lock").exists());
    assert!(InstanceLock::acquire(dir.path()).is_ok());
    let file = dir.path().join("not-directory");
    fs::write(&file, "x").unwrap();
    assert!(matches!(
        InstanceLock::acquire(&file),
        Err(StartupError::Unavailable)
    ));
}
#[test]
fn random_ids_preserve_names_and_no_plaintext_in_files() {
    let (dir, _, app) = fixture();
    let first = add_key(&app);
    let second = add_key(&app);
    assert_ne!(first.id, second.id);
    let providers = list_providers(&app).unwrap();
    assert_eq!(providers[0].name, "用户供应商");
    assert_eq!(providers[0].keys[0].remark, "用户备注");
    for file in fs::read_dir(dir.path())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|f| f.path().is_file() && f.file_name() != "key-switch.lock")
    {
        assert!(!fs::read_to_string(file.path())
            .unwrap()
            .contains("fixture-private-credential"));
    }
}
#[test]
fn sort_requires_exact_permutation() {
    let (_dir, _credentials, app) = fixture();
    for ids in [vec![], vec!["other".into()], vec!["p".into(), "p".into()]] {
        assert_eq!(
            reorder_providers(&app, ids).unwrap_err().code,
            "PROVIDER_ORDER_INVALID"
        );
    }
}
#[test]
fn credential_failures_restore_metadata_and_credentials() {
    // Each stage: backup read/write, new credential write, retired credential deletion.
    for offset in 1..=4 {
        let (_dir, credentials, app) = fixture();
        let key = add_key(&app);
        credentials.fail_next(offset);
        let result = update_api_key(
            &app,
            UpdateKeyInput {
                id: key.id.clone(),
                remark: "changed".into(),
                value: "replacement-fixture".into(),
            },
        );
        assert!(result.is_err());
        let providers = list_providers(&app).unwrap();
        assert_eq!(providers[0].keys[0].remark, "用户备注");
        assert_eq!(credentials.values.lock().unwrap().len(), 1);
        assert_eq!(
            credentials.values.lock().unwrap().values().next().unwrap(),
            "fixture-private-credential"
        );
    }
}
#[test]
fn metadata_failure_rolls_back_new_credential() {
    let (dir, credentials, app) = fixture();
    // Block the metadata temporary path while leaving journal files writable.
    fs::create_dir(dir.path().join("key-switch-data.json.tmp")).unwrap();
    assert!(create_api_key(
        &app,
        CreateKeyInput {
            provider_id: "p".into(),
            remark: String::new(),
            value: "fixture".into()
        }
    )
    .is_err());
    fs::remove_dir(dir.path().join("key-switch-data.json.tmp")).unwrap();
    assert!(list_providers(&app).unwrap()[0].keys.is_empty());
    assert!(credentials.values.lock().unwrap().is_empty());
}
#[test]
fn interrupted_metadata_replacement_validates_candidates() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("key-switch-data.json");
    fs::write(file.with_extension("json.bak"), r#"{"providers":[]}"#).unwrap();
    fs::write(file.with_extension("json.tmp"), "{").unwrap();
    super::runtime::recover_json::<AppData>(&file).unwrap();
    assert!(file.exists());
}
#[test]
fn detection_merge_preserves_edits_and_discards_obsolete_results() {
    let (_dir, _credentials, app) = fixture();
    let key = add_key(&app);
    let snapshot = super::runtime::snapshots(&app, "p", Some(&key.id))
        .unwrap()
        .pop()
        .unwrap();
    update_api_key(
        &app,
        UpdateKeyInput {
            id: key.id.clone(),
            remark: "新备注".into(),
            value: String::new(),
        },
    )
    .unwrap();
    super::runtime::merge_outcomes(
        &app,
        vec![(
            snapshot,
            ValidationOutcome {
                status: "valid",
                error_code: None,
            },
        )],
    )
    .unwrap();
    let data = load_data(&app).unwrap();
    assert_eq!(data.providers[0].keys[0].remark, "新备注");
    assert_eq!(data.providers[0].keys[0].status, "valid");
    let snapshot = super::runtime::snapshots(&app, "p", Some(&key.id))
        .unwrap()
        .pop()
        .unwrap();
    update_api_key(
        &app,
        UpdateKeyInput {
            id: key.id.clone(),
            remark: "保持".into(),
            value: "new-fixture".into(),
        },
    )
    .unwrap();
    super::runtime::merge_outcomes(
        &app,
        vec![(
            snapshot,
            ValidationOutcome {
                status: "invalid",
                error_code: None,
            },
        )],
    )
    .unwrap();
    assert_eq!(
        load_data(&app).unwrap().providers[0].keys[0].status,
        "untested"
    );
    let old = super::runtime::snapshots(&app, "p", Some(&key.id))
        .unwrap()
        .pop()
        .unwrap();
    let new = super::runtime::snapshots(&app, "p", Some(&key.id))
        .unwrap()
        .pop()
        .unwrap();
    super::runtime::merge_outcomes(
        &app,
        vec![(
            new,
            ValidationOutcome {
                status: "valid",
                error_code: None,
            },
        )],
    )
    .unwrap();
    super::runtime::merge_outcomes(
        &app,
        vec![(
            old,
            ValidationOutcome {
                status: "invalid",
                error_code: None,
            },
        )],
    )
    .unwrap();
    assert_eq!(
        load_data(&app).unwrap().providers[0].keys[0].status,
        "valid"
    );
}
#[test]
fn resolves_language_without_settings_io() {
    assert_eq!(resolve_locale(None), Locale::ZhCn);
    assert_eq!(resolve_locale(Some("zh_Hant")), Locale::ZhTw);
    assert_eq!(resolve_locale(Some("fr-FR")), Locale::En);
    assert_eq!(resolve_locale(Some("ja_JP")), Locale::Ja);
}

#[test]
fn tui_does_not_write_settings_and_unicode_directory_is_supported() {
    let parent = tempfile::tempdir().unwrap();
    let directory = parent.path().join("中文 用户目录");
    let core = Core::new(
        InstanceLock::acquire(&directory).unwrap(),
        Arc::new(MockCredentials::default()),
    )
    .unwrap();
    assert_eq!(read_app_settings(&core).unwrap().theme_preference, "system");
    assert!(!directory.join("settings.json").exists());
    save_app_settings(
        &core,
        AppSettings {
            schema_version: 1,
            locale_preference: "ja-JP".into(),
            theme_preference: "dark".into(),
        },
    )
    .unwrap();
    let previous = fs::read(directory.join("settings.json")).unwrap();
    assert_eq!(read_app_settings(&core).unwrap().locale_preference, "ja-JP");
    assert_eq!(fs::read(directory.join("settings.json")).unwrap(), previous);
}

#[test]
fn validates_catalog_and_all_error_translations() {
    let providers = provider_catalog();
    let ids = providers
        .iter()
        .map(|p| &p.id)
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(ids.len(), providers.len());
    for p in &providers {
        assert!(!p.name_zh.is_empty());
        assert!(!p.name_en.is_empty());
        assert!(matches!(
            p.tone.as_str(),
            "blue" | "violet" | "orange" | "indigo" | "gray"
        ));
    }
    let errors: serde_json::Value = serde_json::from_str(include_str!("../errors.json")).unwrap();
    let schema = errors["zh-CN"].as_object().unwrap();
    for language in ["zh-TW", "en-US", "ja-JP"] {
        let translated = errors[language].as_object().unwrap();
        assert_eq!(schema.len(), translated.len());
        for code in schema.keys() {
            assert!(translated[code].as_str().is_some_and(|s| !s.is_empty()));
        }
    }
}

#[test]
fn partial_provider_deletion_restores_every_credential() {
    for offset in 1..=6 {
        let (_dir, credentials, app) = fixture();
        add_key(&app);
        add_key(&app);
        credentials.fail_next(offset);
        assert!(delete_provider(&app, "p".into()).is_err());
        assert_eq!(list_providers(&app).unwrap()[0].keys.len(), 2);
        assert_eq!(credentials.values.lock().unwrap().len(), 2);
    }
}

#[test]
fn copy_calls_adapter_without_returning_a_secret() {
    struct Capture(Mutex<String>);
    impl Clipboard for Capture {
        fn write(&self, value: &str) -> Result<(), CommandError> {
            *self.0.lock().unwrap() = value.into();
            Ok(())
        }
    }
    let (_dir, _credentials, app) = fixture();
    let key = add_key(&app);
    let clipboard = Capture(Mutex::new(String::new()));
    copy_api_key(&app, key.id, &clipboard).unwrap();
    assert_eq!(&*clipboard.0.lock().unwrap(), "fixture-private-credential");
}

#[test]
fn rejects_invalid_custom_images_and_preserves_primary_settings_errors() {
    for logo in [
        "https://example.com/a.png",
        "data:image/svg+xml;base64,PHN2Zz4=",
        "data:image/png;base64,eA==",
    ] {
        assert!(super::runtime::validate_custom_logo(Some(logo)).is_err());
    }
    let (_dir, _credentials, app) = fixture();
    fs::write(settings_file(&app).unwrap(), r#"{"schemaVersion":99}"#).unwrap();
    assert!(matches!(
        read_app_settings(&app),
        Err(CommandError {
            code: "SETTINGS_INVALID"
        })
    ));
}
