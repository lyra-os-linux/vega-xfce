use super::*;
use gtk::prelude::*;
use std::os::unix::fs::symlink;

struct TestHome(PathBuf);

impl TestHome {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "vega-history-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        for name in ["data", "config", "cache"] {
            fs::create_dir(path.join(name)).unwrap();
        }
        Self(path)
    }
}

impl Drop for TestHome {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn messages() -> Vec<Message> {
    vec![Message {
        role: "user".into(),
        content: "private conversation marker".into(),
        attachments: vec![Attachment {
            name: "private.txt".into(),
            mime: "text/plain".into(),
            data: base64_engine.encode(b"private attachment marker"),
        }],
    }]
}

fn persistence(enabled: bool) {
    let settings = crate::preferences::Settings {
        save_ai_history: enabled,
        ..Default::default()
    };
    crate::preferences::save(&settings);
    assert_eq!(crate::preferences::save_ai_history(), enabled);
}

fn legacy_audit() -> String {
    [
        json!({"kind":"user_message","detail":"private conversation marker"}),
        json!({"kind":"assistant_message","detail":"private response marker"}),
        json!({"kind":"user_attachment","detail":"private.txt"}),
        json!({"kind":"mutation_approved","detail":"install fixture"}),
    ]
    .iter()
    .map(|row| format!("{row}\n"))
    .collect()
}

fn run_child(case: &str, test: &str, ignored: bool) {
    let home = TestHome::new();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", test, "--nocapture", "--test-threads=1"])
        .env("VEGA_HISTORY_CASE", case)
        .env("XDG_DATA_HOME", home.0.join("data"))
        .env("XDG_CONFIG_HOME", home.0.join("config"))
        .env("XDG_CACHE_HOME", home.0.join("cache"))
        .env("GSETTINGS_BACKEND", "memory");
    if ignored {
        command.arg("--ignored");
    }
    assert!(
        command.status().unwrap().success(),
        "scenario {case} failed"
    );
}

#[test]
fn history_storage_subprocess() {
    let Ok(case) = std::env::var("VEGA_HISTORY_CASE") else {
        return;
    };
    let dir = data_dir();
    if case == "absent" {
        clear_history().unwrap();
        clear_history().unwrap();
        assert!(!dir.exists(), "deletion should not create private files");
        return;
    }
    persistence(true);
    save_history(&messages()).unwrap();
    assert_eq!(load_history().len(), 1);
    let before = fs::read(dir.join("ai-history.json")).unwrap();
    persistence(false);
    assert!(load_history().is_empty());
    fs::write(dir.join("ai-audit.jsonl"), legacy_audit()).unwrap();
    match case.as_str() {
        "delete" => {
            let original = dir.parent().unwrap().join("original.txt");
            fs::write(&original, b"original attachment must survive").unwrap();
            let attachment = read_attachment(&original).unwrap();
            assert!(!attachment.data.is_empty());
            clear_history().unwrap();
            assert!(!dir.join("ai-history.json").exists());
            assert_eq!(
                fs::read(&original).unwrap(),
                b"original attachment must survive"
            );
            let retained = fs::read_to_string(dir.join("ai-audit.jsonl")).unwrap();
            assert_eq!(
                retained,
                format!(
                    "{}\n",
                    json!({"kind":"mutation_approved","detail":"install fixture"})
                )
            );
            assert_eq!(
                fs::metadata(dir.join("ai-audit.jsonl"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
            persistence(true);
            assert!(load_history().is_empty());
            clear_history().unwrap();
            save_history(&[]).unwrap();
            assert!(load_history().is_empty());
        }
        "malformed_audit" => {
            let invalid = format!("{}broken trailing record", legacy_audit());
            fs::write(dir.join("ai-audit.jsonl"), &invalid).unwrap();
            assert!(clear_history().is_err());
            assert_eq!(fs::read(dir.join("ai-history.json")).unwrap(), before);
            assert_eq!(
                fs::read_to_string(dir.join("ai-audit.jsonl")).unwrap(),
                invalid
            );
            assert!(!fs::read_dir(&dir).unwrap().any(|p| {
                p.unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".ai-audit-clear-")
            }));
        }
        "history_error" => {
            fs::remove_file(dir.join("ai-history.json")).unwrap();
            fs::create_dir(dir.join("ai-history.json")).unwrap();
            fs::write(dir.join("ai-history.json/sentinel"), b"do not recurse").unwrap();
            assert!(clear_history().is_err());
            assert_eq!(
                fs::read(dir.join("ai-history.json/sentinel")).unwrap(),
                b"do not recurse"
            );
        }
        "audit_link" => {
            fs::remove_file(dir.join("ai-audit.jsonl")).unwrap();
            let target = dir.join("other.jsonl");
            fs::write(&target, legacy_audit()).unwrap();
            symlink(&target, dir.join("ai-audit.jsonl")).unwrap();
            assert!(clear_history().is_err());
            assert_eq!(fs::read_to_string(&target).unwrap(), legacy_audit());
            assert_eq!(fs::read(dir.join("ai-history.json")).unwrap(), before);
        }
        "history_link" => {
            fs::remove_file(dir.join("ai-history.json")).unwrap();
            let target = dir.join("original.json");
            fs::write(&target, &before).unwrap();
            symlink(&target, dir.join("ai-history.json")).unwrap();
            clear_history().unwrap();
            assert_eq!(fs::read(target).unwrap(), before);
            assert!(fs::symlink_metadata(dir.join("ai-history.json")).is_err());
        }
        "future_audit" => {
            clear_history().unwrap();
            let retained = fs::read(dir.join("ai-audit.jsonl")).unwrap();
            for kind in ["user_message", "assistant_message", "user_attachment"] {
                audit(kind, "private marker never duplicated").unwrap();
            }
            assert_eq!(fs::read(dir.join("ai-audit.jsonl")).unwrap(), retained);
            audit("mutation_approved", "install next fixture").unwrap();
            assert!(
                fs::read_to_string(dir.join("ai-audit.jsonl"))
                    .unwrap()
                    .contains("install next fixture")
            );
        }
        other => panic!("unknown scenario {other}"),
    }
}

#[test]
fn history_deletion_regressions_with_real_preferences_and_files() {
    for case in [
        "absent",
        "delete",
        "malformed_audit",
        "history_error",
        "audit_link",
        "history_link",
        "future_audit",
    ] {
        run_child(
            case,
            "assistant::history_tests::history_storage_subprocess",
            false,
        );
    }
}

#[test]
#[ignore = "requires a graphical display; CI runs under Xvfb with isolated XDG directories"]
fn native_history_ui() {
    if std::env::var("VEGA_HISTORY_CASE").is_err() {
        run_child("ui", "assistant::history_tests::native_history_ui", true);
        return;
    }
    adw::init().unwrap();
    persistence(true);
    save_history(&messages()).unwrap();
    persistence(false);
    let page = crate::ui::AssistantPage::new(&Settings::default(), messages());
    page.stage_attachment(messages()[0].attachments[0].clone());
    page.prompt.buffer().set_text("pending prompt");
    let context = glib::MainContext::default();
    page.set_busy(true);
    assert!(!page.clear_history.is_sensitive());
    assert!(!page.attach.is_sensitive());
    assert!(context.block_on(page.clear()).is_err());
    assert_eq!(page.history().len(), 1);
    assert!(page.has_staged_attachments());
    page.set_busy(false);
    let path = data_dir().join("ai-history.json");
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(context.block_on(page.clear()).is_err());
    assert!(!page.is_busy());
    assert!(page.clear_history.is_sensitive());
    assert_eq!(page.history().len(), 1);
    assert!(page.has_staged_attachments());
    assert_eq!(page.prompt_text(), "pending prompt");
    fs::remove_dir(&path).unwrap();
    fs::write(&path, serde_json::to_vec(&messages()).unwrap()).unwrap();
    context.block_on(page.clear()).unwrap();
    assert!(page.history().is_empty());
    assert!(!page.has_staged_attachments());
    assert!(page.prompt_text().is_empty());
    assert!(!path.exists());
    persistence(true);
    assert!(load_history().is_empty());
}
