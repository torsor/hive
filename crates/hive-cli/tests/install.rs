use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;

use hive_common::HiveHome;

static INSTALL_HOME_LOCK: Mutex<()> = Mutex::new(());

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn host_install_writes_unit_text() {
    let _lock = INSTALL_HOME_LOCK.lock().unwrap();
    let base = std::env::temp_dir().join(format!("hive-cli-install-{}", std::process::id()));
    std::fs::create_dir_all(&base).unwrap();
    std::env::set_var("HOME", &base);

    let hive = base.join(".hive");
    std::fs::create_dir_all(&hive).unwrap();
    let home = HiveHome { root: hive.clone() };

    let fake_bin = base.join("bin/hive-host");
    std::fs::create_dir_all(fake_bin.parent().unwrap()).unwrap();
    std::fs::write(&fake_bin, b"#!/bin/sh\n").unwrap();

    let path =
        hive_host::install::install(&home, "127.0.0.1", 8788, true, Some(&fake_bin)).unwrap();
    let body = std::fs::read_to_string(&path).unwrap();
    assert!(body.contains("Description=hive-host"));
    assert!(body.contains("127.0.0.1"));
    assert!(body.contains("--port 8788"));
    assert!(body.contains("--dev"));
    assert!(body.contains(&format!("\"{}\"", fake_bin.display())));

    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn host_install_quotes_spaced_home() {
    let _lock = INSTALL_HOME_LOCK.lock().unwrap();
    let base = std::env::temp_dir().join(format!("hive-cli-install-space-{}", std::process::id()));
    std::fs::create_dir_all(&base).unwrap();
    std::env::set_var("HOME", &base);

    let hive = base.join("my hive");
    std::fs::create_dir_all(&hive).unwrap();
    let home = HiveHome { root: hive.clone() };

    let fake_bin = base.join("bin/hive-host");
    std::fs::create_dir_all(fake_bin.parent().unwrap()).unwrap();
    std::fs::write(&fake_bin, b"#!/bin/sh\n").unwrap();

    let path =
        hive_host::install::install(&home, "127.0.0.1", 8788, true, Some(&fake_bin)).unwrap();
    let body = std::fs::read_to_string(&path).unwrap();
    assert!(body.contains("--home \""));
    assert!(body.contains("my hive\""));

    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn install_scripts_parse() {
    let root = repo_root();
    for name in ["install-host.sh", "install-hub.sh"] {
        let script = root.join(name);
        assert!(script.is_file(), "missing {}", script.display());
        let status = Command::new("bash")
            .args(["-n", &script.display().to_string()])
            .status()
            .expect("bash");
        assert!(status.success(), "syntax error in {name}");
        let text = std::fs::read_to_string(&script).unwrap();
        assert!(
            text.contains("--migrate"),
            "{name} should document/pass --migrate"
        );
    }
}

#[test]
fn install_host_script_passes_migrate_only_when_flagged() {
    let text = std::fs::read_to_string(repo_root().join("install-host.sh")).unwrap();
    assert!(text.contains("[[ \"$migrate\" -eq 1 ]] && install_cmd+=\" --migrate\""));
}
