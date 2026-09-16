//! Shared infrastructure for hive-host, hive-hub, hive-cli, and hive-panel.

pub mod bind;
pub mod config;
pub mod home;
pub mod hostname;
pub mod hosts;
pub mod migrate;
pub mod provider_bin;
pub mod systemd;
pub mod task;

pub use bind::{assert_bind_allowed, in_tailnet, tailscale_v4};
pub use config::{ClientToml, HostConfig, HubConfig, WebConfig};
pub use home::{ensure_dir, HiveHome};
pub use hostname::{is_local_hostname, local_short_hostname};
pub use hosts::{host_allowed, parse_host_line, read_host_entries, read_hosts, HostEntry};
pub use migrate::migrate_from_cc;
pub use provider_bin::{operator_path_env, resolve_provider_bin};
pub use systemd::{exec_start, exec_start_web, install_user_service, render_unit, render_unit_with_path, resolve_daemon_bin};
pub use task::validate_task_name;
