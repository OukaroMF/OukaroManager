use std::{
    ffi::{CStr, CString},
    fs,
    path::Path,
    process::Command,
};

use anyhow::{Context, Result};
use regex::Regex;
use rustix::mount::{MountFlags, mount};

pub fn mount_overlyfs<P>(lower: P, upper: P, work: P, target: &str) -> Result<()>
where
    P: AsRef<Path>,
{
    let (lower, upper, work, target) = (lower.as_ref(), upper.as_ref(), work.as_ref(), target);

    fs::create_dir_all(upper).context("create upper")?;
    fs::create_dir_all(work).context("create work")?;
    fs::create_dir_all(target).context("create target")?;

    let opts = format!(
        "lowerdir={lower},upperdir={upper},workdir={work}",
        lower = lower.display(),
        upper = upper.display(),
        work = work.display()
    );
    let opts: Option<&CStr> = Some(&CString::new(opts).unwrap());

    mount("overlay", target, "overlay", MountFlags::empty(), opts)?;
    Ok(())
}

/// Folder Copy
/// from: source folder path
/// to: target path
pub fn dir_copys(from: impl AsRef<Path>, to: impl AsRef<Path>) {
    let output = Command::new("cp")
        .arg("-r")
        .arg(from.as_ref())
        .arg(to.as_ref())
        .output()
        .unwrap();

    if !output.status.success() {
        log::error!(
            "copy files failed: stdout: {}, stderr {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        panic!();
    }
}

/// get packge data path in =/data
/// packge: packge name
pub fn find_data_path(package: &str) -> Result<String> {
    let out = Command::new("pm").args(["path", package]).output()?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let re = Regex::new(r"^package:(.*)").unwrap();
    let caps = match re.captures(&stdout) {
        Some(s) => s,
        None => return Ok(String::new()),
    };
    let mut path = caps[1].to_string();

    path = path.trim_end().trim_end_matches("base.apk").to_string();
    log::info!("{} path is {}", package, path);

    Ok(path)
}
