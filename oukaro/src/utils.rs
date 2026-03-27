use std::{
    collections::{BTreeMap, HashSet},
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result};
use quick_xml::{Reader, events::Event};
#[cfg(any(target_os = "android", target_os = "linux"))]
use rustix::mount::{MountFlags, mount};
#[cfg(any(target_os = "android", target_os = "linux"))]
use std::ffi::{CStr, CString};

use crate::android_install_path::normalize_user_app_code_path;
use crate::android_package_state::SystemUserPackageState;
use crate::android_xml::{parse_boolish, read_xmlish_text};
use crate::defs::{PACKAGES_XML_PATHS, SYSTEM_USER_ID, SYSTEM_USER_PACKAGE_RESTRICTIONS_PATHS};

#[cfg(any(target_os = "android", target_os = "linux"))]
fn is_mount_target(target: &str) -> Result<bool> {
    let mountinfo =
        fs::read_to_string("/proc/self/mountinfo").context("read /proc/self/mountinfo")?;
    Ok(mountinfo
        .lines()
        .filter_map(|line| line.split_whitespace().nth(4))
        .any(|mount_point| mount_point == target))
}

#[cfg(any(target_os = "android", target_os = "linux"))]
pub fn mount_overlyfs<P>(lower: P, upper: P, work: P, target: &str) -> Result<()>
where
    P: AsRef<Path>,
{
    let (lower, upper, work, target) = (lower.as_ref(), upper.as_ref(), work.as_ref(), target);

    fs::create_dir_all(upper).context("create upper")?;
    fs::create_dir_all(work).context("create work")?;
    fs::create_dir_all(target).context("create target")?;

    if is_mount_target(target)? {
        log::info!("overlay already mounted on {target}, skipping");
        return Ok(());
    }

    let opts = format!(
        "lowerdir={lower},upperdir={upper},workdir={work}",
        lower = lower.display(),
        upper = upper.display(),
        work = work.display()
    );
    let opts = CString::new(opts).context("encode overlay mount options")?;
    let opts: Option<&CStr> = Some(opts.as_c_str());

    mount("overlay", target, "overlay", MountFlags::empty(), opts)?;
    Ok(())
}

#[cfg(not(any(target_os = "android", target_os = "linux")))]
pub fn mount_overlyfs<P>(_lower: P, _upper: P, _work: P, target: &str) -> Result<()>
where
    P: AsRef<Path>,
{
    anyhow::bail!("overlay mounting is not supported on this platform: {target}");
}

pub fn sync_package_dir(
    from: impl AsRef<Path>,
    destination_root: impl AsRef<Path>,
    package: &str,
) -> Result<()> {
    let from = from.as_ref();
    let destination_root = destination_root.as_ref();
    let package_dir = destination_root.join(package);
    let staging_dir = destination_root.join(format!(".{package}.tmp"));

    if staging_dir.exists() {
        remove_path(&staging_dir)
            .with_context(|| format!("remove stale staging dir {}", staging_dir.display()))?;
    }

    fs::create_dir_all(&staging_dir)
        .with_context(|| format!("create staging dir {}", staging_dir.display()))?;

    if let Err(error) = copy_dir_contents(from, &staging_dir) {
        let _ = remove_path(&staging_dir);
        return Err(error);
    }

    if package_dir.exists() {
        remove_path(&package_dir)
            .with_context(|| format!("remove existing package dir {}", package_dir.display()))?;
    }

    fs::rename(&staging_dir, &package_dir).with_context(|| {
        format!(
            "rename staging dir {} to {}",
            staging_dir.display(),
            package_dir.display()
        )
    })?;
    Ok(())
}

pub fn cleanup_unmanaged_packages(
    destination_root: impl AsRef<Path>,
    keep_packages: &HashSet<String>,
) -> Result<()> {
    let destination_root = destination_root.as_ref();
    fs::create_dir_all(destination_root)
        .with_context(|| format!("create destination root {}", destination_root.display()))?;

    for entry in fs::read_dir(destination_root)
        .with_context(|| format!("read destination root {}", destination_root.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let package_name = file_name.to_string_lossy();

        if keep_packages.contains(package_name.as_ref()) {
            continue;
        }

        if entry.file_type()?.is_dir() {
            fs::remove_dir_all(&path)
                .with_context(|| format!("remove stale package dir {}", path.display()))?;
        } else {
            fs::remove_file(&path)
                .with_context(|| format!("remove stale file {}", path.display()))?;
        }
    }

    Ok(())
}

fn copy_dir_contents(from: &Path, to: &Path) -> Result<()> {
    for entry in
        fs::read_dir(from).with_context(|| format!("read source dir {}", from.display()))?
    {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = to.join(entry.file_name());
        let metadata = fs::symlink_metadata(&source_path)?;
        let file_type = metadata.file_type();

        if file_type.is_dir() {
            fs::create_dir_all(&destination_path).with_context(|| {
                format!("create destination dir {}", destination_path.display())
            })?;
            copy_dir_contents(&source_path, &destination_path)?;
            let permissions = metadata.permissions();
            fs::set_permissions(&destination_path, permissions)
                .with_context(|| format!("set permissions on {}", destination_path.display()))?;
            continue;
        }

        if file_type.is_symlink() {
            copy_symlink(&source_path, &destination_path)?;
            continue;
        }

        fs::copy(&source_path, &destination_path).with_context(|| {
            format!(
                "copy file from {} to {}",
                source_path.display(),
                destination_path.display()
            )
        })?;
        let permissions = metadata.permissions();
        fs::set_permissions(&destination_path, permissions)
            .with_context(|| format!("set permissions on {}", destination_path.display()))?;
    }

    Ok(())
}

/// get packge data path in =/data
/// packge: packge name
pub fn find_data_path(package: &str) -> Result<Option<PathBuf>> {
    let packages_xml_data_path = match find_data_path_from_packages_xml(package) {
        Ok(Some(data_dir)) => {
            log::info!(
                "{} path is {} (from packages.xml metadata)",
                package,
                data_dir.display()
            );
            Some(data_dir)
        }
        Ok(None) => None,
        Err(error) => {
            log::warn!(
                "failed to resolve package {} from packages.xml metadata: {error:#}",
                package
            );
            None
        }
    };

    match is_installed_for_system_user(package) {
        Ok(true) => {}
        Ok(false) => {
            log::info!(
                "package {} is not installed for system user {}, skipping",
                package,
                SYSTEM_USER_ID
            );
            return Ok(None);
        }
        Err(error) => {
            if let Some(data_path) = packages_xml_data_path {
                log::warn!(
                    "could not confirm package {} for system user {}: {error:#}; using packages.xml code path {} as best-effort fallback",
                    package,
                    SYSTEM_USER_ID,
                    data_path.display()
                );
                return Ok(Some(data_path));
            }

            log::warn!(
                "could not confirm package {} for system user {}: {error:#}; skipping package for safety",
                package,
                SYSTEM_USER_ID
            );
            return Ok(None);
        }
    }

    if let Some(data_dir) = packages_xml_data_path {
        return Ok(Some(data_dir));
    }

    let out = Command::new("pm")
        .args(["path", "--user", SYSTEM_USER_ID, package])
        .output()
        .with_context(|| format!("execute `pm path --user {SYSTEM_USER_ID} {package}`"))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    if !out.status.success() {
        if pm_path_failure_indicates_missing_package(&stdout, &stderr) {
            log::info!(
                "package {} is no longer visible to system user {}, skipping",
                package,
                SYSTEM_USER_ID
            );
            return Ok(None);
        }

        let detail = stderr.trim();
        log::warn!(
            "failed to resolve package {}: {}",
            package,
            if detail.is_empty() {
                "no error output"
            } else {
                detail
            }
        );
        return Ok(None);
    }

    let base_apk = match parse_pm_path_output(&stdout).into_iter().next() {
        Some(path) => path,
        None => return Ok(None),
    };

    let data_dir = base_apk
        .parent()
        .map(Path::to_path_buf)
        .with_context(|| format!("package {package} apk path has no parent"))?;
    log::info!("{} path is {}", package, data_dir.display());

    Ok(Some(data_dir))
}

fn is_installed_for_system_user(package: &str) -> Result<bool> {
    match read_system_user_package_states() {
        Ok(Some(states)) => Ok(states
            .get(package)
            .copied()
            .unwrap_or_default()
            .is_available()),
        Ok(None) => check_package_visible_to_system_user_with_pm(package),
        Err(restrictions_error) => match check_package_visible_to_system_user_with_pm(package) {
            Ok(installed) => {
                log::warn!(
                    "failed to read package restrictions for system user {}: {restrictions_error:#}; using `pm path --user {}` fallback",
                    SYSTEM_USER_ID,
                    SYSTEM_USER_ID
                );
                Ok(installed)
            }
            Err(pm_error) => Err(restrictions_error.context(format!(
                "Fallback `pm path --user {SYSTEM_USER_ID}` probe also failed: {pm_error:#}"
            ))),
        },
    }
}

fn check_package_visible_to_system_user_with_pm(package: &str) -> Result<bool> {
    let out = Command::new("pm")
        .args(["path", "--user", SYSTEM_USER_ID, package])
        .output()
        .with_context(|| format!("execute `pm path --user {SYSTEM_USER_ID} {package}`"))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    if out.status.success() {
        return Ok(!parse_pm_path_output(&stdout).is_empty());
    }

    if pm_path_failure_indicates_missing_package(&stdout, &stderr) {
        return Ok(false);
    }

    let trimmed = stderr.trim();
    let detail = if trimmed.is_empty() {
        "no error output"
    } else {
        trimmed
    };

    anyhow::bail!("`pm path --user {SYSTEM_USER_ID} {package}` failed: {detail}");
}

fn parse_pm_path_output(stdout: &str) -> Vec<PathBuf> {
    stdout
        .lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix("package:"))
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .collect()
}

fn pm_path_failure_indicates_missing_package(stdout: &str, stderr: &str) -> bool {
    if !parse_pm_path_output(stdout).is_empty() {
        return false;
    }

    let trimmed = stderr.trim();
    if trimmed.is_empty() {
        return true;
    }

    let lower = trimmed.to_ascii_lowercase();
    lower.contains("unknown package")
        || lower.contains("package not found")
        || lower.contains("package was not found")
        || lower.contains("unable to find package")
        || lower.contains("not installed for")
}

fn find_data_path_from_packages_xml(package: &str) -> Result<Option<PathBuf>> {
    let mut last_error = None;

    for packages_xml in PACKAGES_XML_PATHS {
        let path = Path::new(packages_xml);
        if !path.exists() {
            continue;
        }

        match find_data_path_from_packages_xml_file(path, package) {
            Ok(data_dir) => return Ok(data_dir),
            Err(error) => {
                log::warn!("failed to inspect {}: {error:#}", path.display());
                last_error = Some(error);
            }
        }
    }

    match last_error {
        Some(error) => Err(error),
        None => Ok(None),
    }
}

fn find_data_path_from_packages_xml_file(path: &Path, package: &str) -> Result<Option<PathBuf>> {
    let contents = read_xmlish_text(path)
        .with_context(|| format!("read package settings {}", path.display()))?;
    if let Some(code_path) = parse_package_code_path(&contents, package)? {
        if let Some(data_dir) = normalize_code_path(code_path) {
            return Ok(Some(data_dir));
        }

        log::warn!(
            "package {} found in {}, but code path is missing on disk",
            package,
            path.display()
        );
    }

    Ok(None)
}

fn read_system_user_package_states() -> Result<Option<BTreeMap<String, SystemUserPackageState>>> {
    let mut last_error = None;

    for restrictions_xml in SYSTEM_USER_PACKAGE_RESTRICTIONS_PATHS {
        let path = Path::new(restrictions_xml);
        if !path.exists() {
            continue;
        }

        match read_package_states_from_restrictions_file(path) {
            Ok(states) => return Ok(Some(states)),
            Err(error) => {
                log::warn!("failed to inspect {}: {error:#}", path.display());
                last_error = Some(error);
            }
        }
    }

    match last_error {
        Some(error) => Err(error),
        None => Ok(None),
    }
}

fn read_package_states_from_restrictions_file(
    path: &Path,
) -> Result<BTreeMap<String, SystemUserPackageState>> {
    let contents = read_xmlish_text(path)
        .with_context(|| format!("read package restrictions {}", path.display()))?;
    parse_package_restrictions_xml(&contents)
        .with_context(|| format!("parse package restrictions {}", path.display()))
}

fn parse_package_code_path(contents: &str, package: &str) -> Result<Option<PathBuf>> {
    let mut reader = Reader::from_str(contents);
    reader.config_mut().trim_text(true);

    loop {
        match reader.read_event()? {
            Event::Start(event) | Event::Empty(event) if event.name().as_ref() == b"package" => {
                let mut package_name = None;
                let mut code_path = None;

                for attribute in event.attributes().with_checks(false) {
                    let attribute = attribute?;
                    let value = attribute
                        .decode_and_unescape_value(reader.decoder())?
                        .into_owned();

                    match attribute.key.as_ref() {
                        b"name" => package_name = Some(value),
                        b"codePath" => code_path = Some(value),
                        _ => {}
                    }
                }

                if package_name.as_deref() == Some(package) {
                    return Ok(code_path.map(PathBuf::from));
                }
            }
            Event::Eof => return Ok(None),
            _ => {}
        }
    }
}

fn parse_package_restrictions_xml(
    contents: &str,
) -> Result<BTreeMap<String, SystemUserPackageState>> {
    let mut reader = Reader::from_str(contents);
    reader.config_mut().trim_text(true);
    let mut states = BTreeMap::new();

    loop {
        match reader.read_event()? {
            Event::Start(event) | Event::Empty(event)
                if event.name().as_ref() == b"pkg" || event.name().as_ref() == b"package" =>
            {
                let mut package_name = None;
                let mut state = SystemUserPackageState::default();

                for attribute in event.attributes().with_checks(false) {
                    let attribute = attribute?;
                    let value = attribute
                        .decode_and_unescape_value(reader.decoder())?
                        .into_owned();

                    match attribute.key.as_ref() {
                        b"name" => package_name = Some(value),
                        b"inst" | b"installed" => {
                            state.installed = parse_boolish(&value).unwrap_or(state.installed)
                        }
                        b"hidden" | b"blocked" => {
                            state.hidden = parse_boolish(&value).unwrap_or(state.hidden)
                        }
                        _ => {}
                    }
                }

                if let Some(package_name) = package_name {
                    states.insert(package_name, state);
                }
            }
            Event::Eof => return Ok(states),
            _ => {}
        }
    }
}

fn normalize_code_path(code_path: PathBuf) -> Option<PathBuf> {
    normalize_user_app_code_path(&code_path)
}

fn remove_path(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => {
            fs::remove_dir_all(path).with_context(|| format!("remove directory {}", path.display()))
        }
        Ok(_) => fs::remove_file(path).with_context(|| format!("remove file {}", path.display())),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("read metadata {}", path.display())),
    }
}

#[cfg(unix)]
fn copy_symlink(source_path: &Path, destination_path: &Path) -> Result<()> {
    use std::os::unix::fs::symlink;

    let target = fs::read_link(source_path)
        .with_context(|| format!("read symlink {}", source_path.display()))?;
    symlink(&target, destination_path).with_context(|| {
        format!(
            "create symlink from {} to {}",
            destination_path.display(),
            target.display()
        )
    })?;
    Ok(())
}

#[cfg(not(unix))]
fn copy_symlink(source_path: &Path, destination_path: &Path) -> Result<()> {
    let target = fs::read_link(source_path)
        .with_context(|| format!("read symlink {}", source_path.display()))?;
    let resolved = source_path
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .join(target);
    fs::copy(&resolved, destination_path).with_context(|| {
        format!(
            "copy symlink target from {} to {}",
            resolved.display(),
            destination_path.display()
        )
    })?;
    Ok(())
}
