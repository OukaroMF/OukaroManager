#[path = "../../shared/android_install_path.rs"]
mod android_install_path;
#[path = "../../shared/android_package.rs"]
mod android_package;
#[path = "../../shared/android_package_state.rs"]
mod android_package_state;
#[path = "../../shared/android_xml.rs"]
mod android_xml;
mod config;
mod defs;
mod utils;

use std::{collections::HashSet, io::Write};

use anyhow::Result;
use env_logger::Builder;

use crate::{
    android_package::validate_package_name,
    defs::{LOWER_PATH, UPPER_PATH, WORK_PATH},
    utils::{cleanup_unmanaged_packages, find_data_path, mount_overlyfs, sync_package_dir},
};

struct ManagedApps {
    system_keep: HashSet<String>,
    priv_keep: HashSet<String>,
    system_apply: Vec<String>,
    priv_apply: Vec<String>,
}

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

fn sanitize_managed_apps(apps: config::App) -> ManagedApps {
    let mut priv_keep = HashSet::new();
    for package in apps.priv_app {
        if let Err(error) = validate_package_name(&package) {
            log::warn!("skipping invalid priv-app config entry `{package}`: {error}");
            continue;
        }

        priv_keep.insert(package);
    }

    let mut system_keep = HashSet::new();
    for package in apps.system_app {
        if let Err(error) = validate_package_name(&package) {
            log::warn!("skipping invalid system-app config entry `{package}`: {error}");
            continue;
        }

        if priv_keep.contains(&package) {
            log::warn!(
                "package `{package}` is configured in both priv-app and system-app; preferring priv-app"
            );
            continue;
        }

        system_keep.insert(package);
    }

    let mut priv_apply = priv_keep.iter().cloned().collect::<Vec<_>>();
    priv_apply.sort();

    let mut system_apply = system_keep.iter().cloned().collect::<Vec<_>>();
    system_apply.sort();

    ManagedApps {
        system_keep,
        priv_keep,
        system_apply,
        priv_apply,
    }
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
    let managed_apps = sanitize_managed_apps(config.get());

    cleanup_unmanaged_packages(&priv_root, &managed_apps.priv_keep)?;
    cleanup_unmanaged_packages(&system_root, &managed_apps.system_keep)?;

    log::info!("handling system/priv-app");
    for app in managed_apps.priv_apply {
        match find_data_path(&app)? {
            Some(data_path) => {
                sync_package_dir(data_path, &priv_root, &app)?;
                log::info!("synced priv-app package {app}");
            }
            None => log::warn!("package {app} is not installed; keeping config entry only"),
        }
    }

    log::info!("handling system/app");
    for app in managed_apps.system_apply {
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::config::App;

    use super::sanitize_managed_apps;

    #[test]
    fn sanitize_managed_apps_skips_invalid_entries_and_prefers_priv_app() {
        let managed = sanitize_managed_apps(App {
            system_app: HashSet::from([
                "com.example.shared".to_string(),
                "com.example.system".to_string(),
                "../escape".to_string(),
            ]),
            priv_app: HashSet::from([
                "com.example.shared".to_string(),
                "com.example.priv".to_string(),
                "single".to_string(),
            ]),
        });

        assert_eq!(
            managed.priv_apply,
            vec![
                "com.example.priv".to_string(),
                "com.example.shared".to_string(),
            ]
        );
        assert_eq!(managed.system_apply, vec!["com.example.system".to_string()]);
        assert!(managed.priv_keep.contains("com.example.shared"));
        assert!(!managed.system_keep.contains("com.example.shared"));
    }
}
