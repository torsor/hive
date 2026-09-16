use std::path::Path;

use anyhow::Result;
use hive_common::{
    assert_bind_allowed, ensure_dir, exec_start, install_user_service, render_unit,
    resolve_daemon_bin, HiveHome,
};

pub fn install(
    home: &HiveHome,
    bind: &str,
    port: u16,
    dev: bool,
    bin: Option<&Path>,
) -> Result<std::path::PathBuf> {
    assert_bind_allowed("hive-hub", bind, dev)?;
    ensure_dir(&home.root)?;
    let bin = resolve_daemon_bin(bin, "HIVE_HUB_BIN")?;
    let exec = exec_start(&bin, bind, port, &home.root.display().to_string(), dev);
    let unit = render_unit("hive-hub (fleet aggregator)", &exec);
    install_user_service("hive-hub.service", &unit)
}
