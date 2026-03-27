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

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, HashSet},
        fs,
        path::PathBuf,
    };

    use tempfile::tempdir;

    use super::{
        cleanup_unmanaged_packages, find_data_path_from_packages_xml_file,
        parse_package_restrictions_xml, parse_pm_path_output,
        pm_path_failure_indicates_missing_package, read_package_states_from_restrictions_file,
        sync_package_dir,
    };
    use crate::android_package_state::SystemUserPackageState;

    #[test]
    fn sync_package_dir_copies_contents_into_named_folder() {
        let source_dir = tempdir().unwrap();
        let nested_dir = source_dir.path().join("lib");
        let destination_dir = tempdir().unwrap();

        fs::create_dir_all(&nested_dir).unwrap();
        fs::write(source_dir.path().join("base.apk"), b"apk").unwrap();
        fs::write(nested_dir.join("split_config.apk"), b"split").unwrap();

        sync_package_dir(source_dir.path(), destination_dir.path(), "com.example.app").unwrap();

        let package_dir = destination_dir.path().join("com.example.app");
        assert_eq!(fs::read(package_dir.join("base.apk")).unwrap(), b"apk");
        assert_eq!(
            fs::read(package_dir.join("lib").join("split_config.apk")).unwrap(),
            b"split"
        );
    }

    #[test]
    fn cleanup_unmanaged_packages_removes_directories_not_in_config() {
        let destination_dir = tempdir().unwrap();
        fs::create_dir_all(destination_dir.path().join("keep.me")).unwrap();
        fs::create_dir_all(destination_dir.path().join("remove.me")).unwrap();

        cleanup_unmanaged_packages(
            destination_dir.path(),
            &HashSet::from([String::from("keep.me")]),
        )
        .unwrap();

        assert!(destination_dir.path().join("keep.me").exists());
        assert!(!destination_dir.path().join("remove.me").exists());
    }

    #[test]
    fn packages_xml_lookup_supports_directory_code_paths() {
        let root = tempdir().unwrap();
        let package_dir = root
            .path()
            .join("data")
            .join("app")
            .join("com.example.alpha");
        fs::create_dir_all(&package_dir).unwrap();

        let packages_xml = root.path().join("packages.xml");
        fs::write(
            &packages_xml,
            format!(
                r#"<packages><package name="com.example.alpha" codePath="{}" /></packages>"#,
                package_dir.display()
            ),
        )
        .unwrap();

        let resolved =
            find_data_path_from_packages_xml_file(&packages_xml, "com.example.alpha").unwrap();

        assert_eq!(resolved, Some(package_dir));
    }

    #[test]
    fn packages_xml_lookup_supports_base_apk_code_paths() {
        let root = tempdir().unwrap();
        let package_dir = root
            .path()
            .join("data")
            .join("app")
            .join("com.example.beta");
        fs::create_dir_all(&package_dir).unwrap();
        let base_apk = package_dir.join("base.apk");
        fs::write(&base_apk, b"apk").unwrap();

        let packages_xml = root.path().join("packages.xml");
        fs::write(
            &packages_xml,
            format!(
                r#"<packages><package name="com.example.beta" codePath="{}" /></packages>"#,
                base_apk.display()
            ),
        )
        .unwrap();

        let resolved =
            find_data_path_from_packages_xml_file(&packages_xml, "com.example.beta").unwrap();

        assert_eq!(resolved, Some(package_dir));
    }

    #[test]
    fn packages_xml_lookup_rejects_system_partition_code_paths_even_if_they_exist() {
        let root = tempdir().unwrap();
        let package_dir = root.path().join("system").join("app").join("System");
        fs::create_dir_all(&package_dir).unwrap();
        let base_apk = package_dir.join("System.apk");
        fs::write(&base_apk, b"apk").unwrap();

        let packages_xml = root.path().join("packages.xml");
        fs::write(
            &packages_xml,
            format!(
                r#"<packages><package name="com.example.system" codePath="{}" /></packages>"#,
                base_apk.display()
            ),
        )
        .unwrap();

        let resolved =
            find_data_path_from_packages_xml_file(&packages_xml, "com.example.system").unwrap();

        assert_eq!(resolved, None);
    }

    #[test]
    fn current_packages_xml_missing_package_is_not_treated_as_backup_hit() {
        let root = tempdir().unwrap();
        let current = root.path().join("packages.xml");
        let backup = root.path().join("packages-backup.xml");
        let package_dir = root
            .path()
            .join("data")
            .join("app")
            .join("com.example.stale");
        fs::create_dir_all(&package_dir).unwrap();

        fs::write(&current, "<packages></packages>").unwrap();
        fs::write(
            &backup,
            format!(
                r#"<packages><package name="com.example.stale" codePath="{}" /></packages>"#,
                package_dir.display()
            ),
        )
        .unwrap();

        let current_resolved =
            find_data_path_from_packages_xml_file(&current, "com.example.stale").unwrap();
        let backup_resolved =
            find_data_path_from_packages_xml_file(&backup, "com.example.stale").unwrap();

        assert_eq!(current_resolved, None);
        assert_eq!(backup_resolved, Some(package_dir));
    }

    #[test]
    fn invalid_current_packages_xml_returns_error() {
        let root = tempdir().unwrap();
        let current = root.path().join("packages.xml");
        fs::write(&current, "<packages><package").unwrap();

        assert!(find_data_path_from_packages_xml_file(&current, "com.example.app").is_err());
    }

    #[test]
    fn packages_xml_lookup_supports_android_binary_xml() {
        let root = tempdir().unwrap();
        let package_dir = root
            .path()
            .join("data")
            .join("app")
            .join("com.example.binary");
        fs::create_dir_all(&package_dir).unwrap();

        let packages_xml = root.path().join("packages.xml");
        let mut abx = Vec::new();
        abx.extend_from_slice(b"ABX\0");
        abx.push(0x00);

        abx.push(0x02);
        abx.extend_from_slice(&0xFFFF_u16.to_be_bytes());
        abx.extend_from_slice(&(8_u16).to_be_bytes());
        abx.extend_from_slice(b"packages");

        abx.push(0x02);
        abx.extend_from_slice(&0xFFFF_u16.to_be_bytes());
        abx.extend_from_slice(&(7_u16).to_be_bytes());
        abx.extend_from_slice(b"package");

        abx.push(0x2F);
        abx.extend_from_slice(&0xFFFF_u16.to_be_bytes());
        abx.extend_from_slice(&(4_u16).to_be_bytes());
        abx.extend_from_slice(b"name");
        abx.extend_from_slice(&(18_u16).to_be_bytes());
        abx.extend_from_slice(b"com.example.binary");

        abx.push(0x2F);
        abx.extend_from_slice(&0xFFFF_u16.to_be_bytes());
        abx.extend_from_slice(&(8_u16).to_be_bytes());
        abx.extend_from_slice(b"codePath");
        let code_path = package_dir.display().to_string();
        abx.extend_from_slice(&(code_path.len() as u16).to_be_bytes());
        abx.extend_from_slice(code_path.as_bytes());

        abx.push(0x03);
        abx.extend_from_slice(&1_u16.to_be_bytes());
        abx.push(0x03);
        abx.extend_from_slice(&0_u16.to_be_bytes());
        abx.push(0x01);

        fs::write(&packages_xml, abx).unwrap();

        let resolved =
            find_data_path_from_packages_xml_file(&packages_xml, "com.example.binary").unwrap();

        assert_eq!(resolved, Some(package_dir));
    }

    #[test]
    fn package_restrictions_parser_reads_installed_state_for_system_user() {
        let parsed = parse_package_restrictions_xml(
            r#"
            <package-restrictions>
              <pkg name="com.example.alpha" inst="true" />
              <pkg name="com.example.beta" inst="false" />
              <package name="com.example.gamma" installed="false" />
              <pkg name="com.example.delta" />
            </package-restrictions>
            "#,
        )
        .unwrap();

        assert_eq!(
            parsed,
            BTreeMap::from([
                (
                    "com.example.alpha".to_string(),
                    SystemUserPackageState {
                        installed: true,
                        hidden: false,
                    }
                ),
                (
                    "com.example.beta".to_string(),
                    SystemUserPackageState {
                        installed: false,
                        hidden: false,
                    }
                ),
                (
                    "com.example.delta".to_string(),
                    SystemUserPackageState {
                        installed: true,
                        hidden: false,
                    }
                ),
                (
                    "com.example.gamma".to_string(),
                    SystemUserPackageState {
                        installed: false,
                        hidden: false,
                    }
                ),
            ])
        );
    }

    #[test]
    fn package_restrictions_parser_reads_hidden_and_legacy_blocked_states() {
        let parsed = parse_package_restrictions_xml(
            r#"
            <package-restrictions>
              <pkg name="com.example.hidden" hidden="true" />
              <pkg name="com.example.blocked" blocked="true" />
            </package-restrictions>
            "#,
        )
        .unwrap();

        assert_eq!(
            parsed,
            BTreeMap::from([
                (
                    "com.example.blocked".to_string(),
                    SystemUserPackageState {
                        installed: true,
                        hidden: true,
                    }
                ),
                (
                    "com.example.hidden".to_string(),
                    SystemUserPackageState {
                        installed: true,
                        hidden: true,
                    }
                ),
            ])
        );
    }

    #[test]
    fn package_restrictions_reader_supports_android_binary_xml() {
        let root = tempdir().unwrap();
        let restrictions = root.path().join("package-restrictions.xml");

        let mut abx = Vec::new();
        abx.extend_from_slice(b"ABX\0");
        abx.push(0x00);

        abx.push(0x02);
        abx.extend_from_slice(&0xFFFF_u16.to_be_bytes());
        abx.extend_from_slice(&(20_u16).to_be_bytes());
        abx.extend_from_slice(b"package-restrictions");

        abx.push(0x02);
        abx.extend_from_slice(&0xFFFF_u16.to_be_bytes());
        abx.extend_from_slice(&(3_u16).to_be_bytes());
        abx.extend_from_slice(b"pkg");

        abx.push(0x2F);
        abx.extend_from_slice(&0xFFFF_u16.to_be_bytes());
        abx.extend_from_slice(&(4_u16).to_be_bytes());
        abx.extend_from_slice(b"name");
        abx.extend_from_slice(&(17_u16).to_be_bytes());
        abx.extend_from_slice(b"com.example.alpha");

        abx.push(0xDF);
        abx.extend_from_slice(&0xFFFF_u16.to_be_bytes());
        abx.extend_from_slice(&(4_u16).to_be_bytes());
        abx.extend_from_slice(b"inst");

        abx.push(0x03);
        abx.extend_from_slice(&1_u16.to_be_bytes());
        abx.push(0x03);
        abx.extend_from_slice(&0_u16.to_be_bytes());
        abx.push(0x01);

        fs::write(&restrictions, abx).unwrap();

        let states = read_package_states_from_restrictions_file(&restrictions).unwrap();

        assert_eq!(
            states,
            BTreeMap::from([(
                "com.example.alpha".to_string(),
                SystemUserPackageState {
                    installed: false,
                    hidden: false,
                }
            )])
        );
    }

    #[test]
    fn pm_path_parser_extracts_base_and_split_paths() {
        let paths = parse_pm_path_output(
            "package:/data/app/~~abc/com.example/base.apk\npackage:/data/app/~~abc/com.example/split_config.arm64_v8a.apk\n",
        );

        assert_eq!(
            paths,
            vec![
                PathBuf::from("/data/app/~~abc/com.example/base.apk"),
                PathBuf::from("/data/app/~~abc/com.example/split_config.arm64_v8a.apk"),
            ]
        );
    }

    #[test]
    fn pm_path_missing_package_without_stderr_matches_aosp_shell_behavior() {
        assert!(pm_path_failure_indicates_missing_package("", ""));
        assert!(pm_path_failure_indicates_missing_package(
            "",
            "Error: Unknown package: com.example.missing"
        ));
    }

    #[test]
    fn pm_path_service_failures_are_not_treated_as_missing_package() {
        assert!(!pm_path_failure_indicates_missing_package(
            "",
            "cmd: Can't find service: package"
        ));
    }
}
