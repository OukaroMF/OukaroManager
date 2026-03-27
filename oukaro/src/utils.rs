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
    let opts: Option<&CStr> = Some(&CString::new(opts).unwrap());

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
            log::warn!(
                "could not confirm package {} for system user {}: {error:#}; skipping package for safety",
                package,
                SYSTEM_USER_ID
            );
            return Ok(None);
        }
    }

    match find_data_path_from_packages_xml(package) {
        Ok(Some(data_dir)) => {
            log::info!(
                "{} path is {} (from packages.xml)",
                package,
                data_dir.display()
            );
            return Ok(Some(data_dir));
        }
        Ok(None) => {}
        Err(error) => {
            log::warn!(
                "failed to resolve package {} from packages.xml metadata: {error:#}",
                package
            );
        }
    }

    let out = Command::new("pm")
        .args(["path", "--user", SYSTEM_USER_ID, package])
        .output()
        .with_context(|| format!("execute `pm path --user {SYSTEM_USER_ID} {package}`"))?;

    if !out.status.success() {
        log::warn!(
            "failed to resolve package {}: {}",
            package,
            String::from_utf8_lossy(&out.stderr).trim()
        );
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&out.stdout);
    let base_apk = match stdout
        .lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix("package:"))
    {
        Some(path) if !path.is_empty() => PathBuf::from(path),
        _ => return Ok(None),
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
        Ok(Some(states)) => Ok(states.get(package).copied().unwrap_or(true)),
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

    if out.status.success() {
        let stdout = String::from_utf8_lossy(&out.stdout);
        return Ok(stdout
            .lines()
            .map(str::trim)
            .any(|line| line.starts_with("package:")));
    }

    let stderr = String::from_utf8_lossy(&out.stderr);
    let trimmed = stderr.trim();
    if trimmed.contains("not found") || trimmed.contains("Unknown package") {
        return Ok(false);
    }

    anyhow::bail!("`pm path --user {SYSTEM_USER_ID} {package}` failed: {trimmed}");
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
    let contents = fs::read_to_string(path)
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

fn read_system_user_package_states() -> Result<Option<BTreeMap<String, bool>>> {
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

fn read_package_states_from_restrictions_file(path: &Path) -> Result<BTreeMap<String, bool>> {
    let contents = fs::read_to_string(path)
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

fn parse_package_restrictions_xml(contents: &str) -> Result<BTreeMap<String, bool>> {
    let mut reader = Reader::from_str(contents);
    reader.config_mut().trim_text(true);
    let mut states = BTreeMap::new();

    loop {
        match reader.read_event()? {
            Event::Start(event) | Event::Empty(event)
                if event.name().as_ref() == b"pkg" || event.name().as_ref() == b"package" =>
            {
                let mut package_name = None;
                let mut installed = true;

                for attribute in event.attributes().with_checks(false) {
                    let attribute = attribute?;
                    let value = attribute
                        .decode_and_unescape_value(reader.decoder())?
                        .into_owned();

                    match attribute.key.as_ref() {
                        b"name" => package_name = Some(value),
                        b"inst" | b"installed" => installed = !value.eq_ignore_ascii_case("false"),
                        _ => {}
                    }
                }

                if let Some(package_name) = package_name {
                    states.insert(package_name, installed);
                }
            }
            Event::Eof => return Ok(states),
            _ => {}
        }
    }
}

fn normalize_code_path(code_path: PathBuf) -> Option<PathBuf> {
    if code_path.is_dir() {
        return Some(code_path);
    }

    if code_path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("apk"))
    {
        return code_path
            .parent()
            .filter(|parent| parent.exists())
            .map(Path::to_path_buf);
    }

    if code_path.is_file() {
        return code_path.parent().map(Path::to_path_buf);
    }

    None
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
    };

    use tempfile::tempdir;

    use super::{
        cleanup_unmanaged_packages, find_data_path_from_packages_xml_file,
        parse_package_restrictions_xml, sync_package_dir,
    };

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
                ("com.example.alpha".to_string(), true),
                ("com.example.beta".to_string(), false),
                ("com.example.delta".to_string(), true),
                ("com.example.gamma".to_string(), false),
            ])
        );
    }
}
