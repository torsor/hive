//! Session listing tests.

use hive_common::HiveHome;
use hive_host::sessions;

#[test]
fn list_sessions_reads_hive_labels_sidecar() {
    let dir = std::env::temp_dir().join(format!("hive-sessions-labels-{}", std::process::id()));
    std::env::set_var("HOME", &dir);
    let home = HiveHome {
        root: dir.join(".hive"),
    };
    std::fs::create_dir_all(home.sessions_dir()).unwrap();
    std::fs::write(
        home.sessions_dir().join("demo.meta"),
        "task='demo'\ndir='/tmp'\ntmux='hive-demo'\nprovider='claude'\n",
    )
    .unwrap();
    std::fs::write(
        home.sessions_dir().join("demo.labels"),
        "{\"starred\":true,\"tags\":[]}\n",
    )
    .unwrap();

    let rows = sessions::list_sessions(&home);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].task, "demo");
    assert!(rows[0].starred);

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn list_sessions_ignores_cc_sessions_dir() {
    let dir = std::env::temp_dir().join(format!("hive-sessions-cc-ignore-{}", std::process::id()));
    std::env::set_var("HOME", &dir);
    let legacy = dir.join(".cc/sessions");
    std::fs::create_dir_all(&legacy).unwrap();
    std::fs::write(
        legacy.join("cc-only.meta"),
        "task='cc-only'\ndir='/tmp'\ntmux='cc-cc-only'\nprovider='claude'\n",
    )
    .unwrap();

    let home = HiveHome {
        root: dir.join(".hive"),
    };
    std::fs::create_dir_all(home.sessions_dir()).unwrap();

    let rows = sessions::list_sessions(&home);
    assert!(rows.is_empty());

    let _ = std::fs::remove_dir_all(dir);
}
