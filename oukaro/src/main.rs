mod config;
mod defs;
mod utils;

use std::io::Write;

use anyhow::Result;
use env_logger::Builder;

use crate::{
    defs::{LOWER_PATH, UPPER_PATH, WORK_PATH},
    utils::{cleanup_unmanaged_packages, find_data_path, mount_overlyfs, sync_package_dir},
};

fn init_logger() {
    let mut builder = Builder::new();
    builder.format(|buf, record| {
        let local_time = chrono::Local::now();
        let time_str = local_time.format("%Y-%m-%d %H:%M:%S%.3f").to_string();

        writeln!(
            buf,
            "[{}] [{}] {} {}",
            time_str,
            record.level(),
            record.target(),
            record.args()
        )
    });
    builder.filter_level(log::LevelFilter::Info).init();
}

fn apply_saved_config() -> Result<()> {
    let mut config = config::Config::new();
    let lower = std::path::Path::new(LOWER_PATH);
    let upper = std::path::Path::new(UPPER_PATH);
    let work = std::path::Path::new(WORK_PATH);
    let system_root = upper.join("app");
    let priv_root = upper.join("priv-app");

    mount_overlyfs(
        lower.join("priv-app"),
        upper.join("priv-app"),
        work.join("priv-app"),
        "/system/priv-app",
    )?;
    mount_overlyfs(
        lower.join("app"),
        upper.join("app"),
        work.join("app"),
        "/system/app",
    )?;

    config.load_config()?;
    let apps = config.get();

    cleanup_unmanaged_packages(&priv_root, &apps.priv_app)?;
    cleanup_unmanaged_packages(&system_root, &apps.system_app)?;

    log::info!("handling system/priv-app");
    for app in apps.priv_app {
        match find_data_path(&app)? {
            Some(data_path) => {
                sync_package_dir(data_path, &priv_root, &app)?;
                log::info!("synced priv-app package {app}");
            }
            None => log::warn!("package {app} is not installed; keeping config entry only"),
        }
    }

    log::info!("handling system/app");
    for app in apps.system_app {
        match find_data_path(&app)? {
            Some(data_path) => {
                sync_package_dir(data_path, &system_root, &app)?;
                log::info!("synced system-app package {app}");
            }
            None => log::warn!("package {app} is not installed; keeping config entry only"),
        }
    }

    Ok(())
}

fn run() -> Result<()> {
    init_logger();
    log::info!("applying saved config during boot");
    apply_saved_config()
}

fn main() {
    run().unwrap_or_else(|e| {
        for c in e.chain() {
            eprintln!("{c:#?}");
        }
        eprintln!("{:#?}", e.backtrace());
    })
}
