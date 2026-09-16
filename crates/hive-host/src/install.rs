use std::path::Path;

use anyhow::Result;
use hive_common::{
    assert_bind_allowed, ensure_dir, exec_start, install_user_service, render_unit_with_path,
    resolve_daemon_bin, HiveHome,
};

pub use hive_common::migrate_from_cc;

pub fn install(
    home: &HiveHome,
    bind: &str,
    port: u16,
    dev: bool,
    bin: Option<&Path>,
) -> Result<std::path::PathBuf> {
    assert_bind_allowed("hive-host", bind, dev)?;
    ensure_dir(&home.root)?;
    ensure_dir(&home.sessions_dir())?;
    let bin = resolve_daemon_bin(bin, "HIVE_HOST_BIN")?;
    let exec = exec_start(&bin, bind, port, &home.root.display().to_string(), dev);
    let operator = home.operator_home()?;
    let path = hive_common::operator_path_env(&operator);
    let unit = render_unit_with_path("hive-host (per-box hive daemon)", &exec, Some(&path));
    install_user_service("hive-host.service", &unit)
}
