use std::{ffi::CString, fs, os::unix::ffi::OsStrExt, path::Path, process::Command};

use anyhow::{Context, Result};
use libc::{MS_NODEV, MS_NOEXEC, MS_NOSUID, MS_RDONLY};
use regex::Regex;

pub fn mount_overlyfs<P>(lower: P, upper: P, work: P, target: &str) -> Result<()>
where
    P: AsRef<Path>,
{
    let (lower, upper, work, target) = (
        lower.as_ref(),
        upper.as_ref(),
        work.as_ref(),
        target.as_ref(),
    );

    fs::create_dir_all(upper).context("create upper")?;
    fs::create_dir_all(work).context("create work")?;
    fs::create_dir_all(target).context("create target")?;

    let opt_bytes = format!(
        "lowerdir={lower},upperdir={upper},workdir={work}",
        lower = lower.display(),
        upper = upper.display(),
        work = work.display()
    );
    let fstype = CString::new("overlay").unwrap();
    let source = CString::new("overlay").unwrap();
    let opts = CString::new(opt_bytes).unwrap();

    unsafe {
        let ret = libc::mount(
            source.as_ptr(),
            target.as_os_str().as_bytes().as_ptr() as *const _,
            fstype.as_ptr(),
            (MS_NOSUID | MS_NODEV | MS_NOEXEC | MS_RDONLY) as libc::c_ulong,
            opts.as_ptr() as *const libc::c_void,
        );
        if ret != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
    }
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
    let out = Command::new("pm").args(&["path", package]).output()?;
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
