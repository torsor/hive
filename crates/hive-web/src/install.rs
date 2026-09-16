use std::path::Path;

use anyhow::Result;
use hive_common::{
    ensure_dir, exec_start_web, install_user_service, render_unit, resolve_daemon_bin, HiveHome,
};

pub fn install(
    home: &HiveHome,
    bind: &str,
    port: u16,
    root: &str,
    dev: bool,
    bin: Option<&Path>,
) -> Result<std::path::PathBuf> {
    ensure_dir(&home.root)?;
    ensure_dir(Path::new(root))?;
    let bin = resolve_daemon_bin(bin, "HIVE_WEB_BIN")?;
    let exec = exec_start_web(&bin, bind, port, &home.root.display().to_string(), root, dev);
    let unit = render_unit("hive-web (phone console static)", &exec);
    install_user_service("hive-web.service", &unit)
}
