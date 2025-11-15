mod config;
mod defs;
mod utils;

use std::{io::Write, path::Path};

use anyhow::Result;
use env_logger::Builder;
use inotify::{Inotify, WatchMask};

use crate::{
    defs::{LOWER_PATH, SYSTEM_PATH, UPPER_PATH, WORK_PATH},
    utils::{dir_copys, find_data_path, mount_overlyfs},
};

fn run() -> Result<()> {
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

    let mut config = config::Config::new();
    let mut inotify = Inotify::init()?;
    let system_path = Path::new("/system");

    inotify.watches().add(SYSTEM_PATH, WatchMask::MODIFY)?;
    mount_overlyfs(LOWER_PATH, UPPER_PATH, WORK_PATH, "/system/")?;

    loop {
        config.load_config()?;

        let apps = config.get();

        log::info!("handling system/priv-app");
        for app in apps.priv_app {
            let data_path = find_data_path(&app)?;

            dir_copys(data_path, system_path.join("priv-app"));
            log::info!("mount successful.")
        }

        log::info!("handling system/app");
        for app in apps.system_app {
            let data_path = find_data_path(&app)?;

            dir_copys(data_path, system_path.join("app"));
            log::info!("mount successful.")
        }
        inotify.read_events_blocking(&mut [0; 1024])?;
    }
}

fn main() {
    run().unwrap_or_else(|e| {
        for c in e.chain() {
            eprintln!("{c:#?}");
        }
        eprintln!("{:#?}", e.backtrace());
    })
}
