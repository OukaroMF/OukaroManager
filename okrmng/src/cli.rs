use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    process::Command,
};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use quick_xml::{Reader, events::Event};
use serde::Serialize;

use crate::android_install_path::has_known_user_app_prefix;
use crate::android_package::{is_valid_package_name, validate_package_name};
use crate::android_package_state::SystemUserPackageState;
use crate::android_xml::{parse_boolish, parse_i64ish, read_xmlish_text};
use crate::config::{App, Config};
use crate::defs::{PACKAGES_XML_PATHS, SYSTEM_USER_ID, SYSTEM_USER_PACKAGE_RESTRICTIONS_PATHS};

const APPLICATION_INFO_FLAG_SYSTEM: i64 = 1 << 0;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum InstalledUserAppsSource {
    PmListPackages,
    PackagesXmlAndRestrictions,
    PackagesXmlBestEffort,
}

impl InstalledUserAppsSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::PmListPackages => "pmListPackages",
            Self::PackagesXmlAndRestrictions => "packagesXmlAndRestrictions",
            Self::PackagesXmlBestEffort => "packagesXmlBestEffort",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum SystemUserStateSource {
    PackageRestrictions,
}

impl SystemUserStateSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::PackageRestrictions => "packageRestrictions",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InstalledUserAppsListing {
    packages: BTreeSet<String>,
    source: InstalledUserAppsSource,
    system_user_state_source: Option<SystemUserStateSource>,
    warnings: Vec<String>,
}

#[derive(Parser)]
#[command(author, version = "0.1", about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum SystemApp {
    Add {
        #[arg(long, short)]
        package: String,
    },
    Rm {
        #[arg(long, short)]
        package: String,
    },
}

#[derive(Subcommand)]
enum PrivApp {
    Add {
        #[arg(long, short)]
        package: String,
    },
    Rm {
        #[arg(long, short)]
        package: String,
    },
}

#[derive(Subcommand)]
enum Commands {
    SystemApp {
        #[command(subcommand)]
        command: SystemApp,
    },
    PrivApp {
        #[command(subcommand)]
        command: PrivApp,
    },
    Inspect {
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    Replace {
        #[arg(long, default_value = "")]
        system: String,
        #[arg(long = "priv", default_value = "")]
        priv_app: String,
    },
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct InspectOutput {
    system_app: Vec<String>,
    priv_app: Vec<String>,
    installed_user_apps: Vec<String>,
    missing_configured_apps: Vec<String>,
    installed_user_apps_source: InstalledUserAppsSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_user_state_source: Option<SystemUserStateSource>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    warnings: Vec<String>,
}

pub fn run() -> Result<()> {
    let args = Args::parse();

    match args.command {
        Commands::PrivApp { command } => {
            println!("setting priv-app");
            let mut config = Config::new()?;

            match command {
                PrivApp::Add { package } => {
                    validate_package_name(&package)?;
                    ensure_not_in_other_group(&config.app.system_app, &package, "system-app")?;
                    config.app.priv_app.insert(package);
                    println!("added new package");
                }
                PrivApp::Rm { package } => {
                    config.app.priv_app.remove(&package);
                    println!("removed package");
                }
            }

            config.save()?;
        }
        Commands::SystemApp { command } => {
            println!("setting system-app");
            let mut config = Config::new()?;

            match command {
                SystemApp::Add { package } => {
                    validate_package_name(&package)?;
                    ensure_not_in_other_group(&config.app.priv_app, &package, "priv-app")?;
                    config.app.system_app.insert(package);
                    println!("added new package");
                }
                SystemApp::Rm { package } => {
                    config.app.system_app.remove(&package);
                    println!("removed package");
                }
            }

            config.save()?;
        }
        Commands::Inspect { json } => {
            let config = Config::new()?;
            let installed_user_apps = list_installed_user_apps()?;
            let inspect = build_inspect_output(&config.app, &installed_user_apps);

            if json {
                println!(
                    "{}",
                    serde_json::to_string(&inspect)
                        .context("Failed to serialize inspect output")?
                );
            } else {
                println!("system_app={}", inspect.system_app.join(","));
                println!("priv_app={}", inspect.priv_app.join(","));
                println!(
                    "installed_user_apps={}",
                    inspect.installed_user_apps.join(",")
                );
                println!(
                    "installed_user_apps_source={}",
                    inspect.installed_user_apps_source.as_str()
                );
                if let Some(source) = inspect.system_user_state_source {
                    println!("system_user_state_source={}", source.as_str());
                }
                println!(
                    "missing_configured_apps={}",
                    inspect.missing_configured_apps.join(",")
                );
                for warning in &inspect.warnings {
                    eprintln!("warning: {warning}");
                }
            }
        }
        Commands::Replace { system, priv_app } => {
            let mut config = Config::new()?;
            let system_packages = parse_package_csv(&system)?;
            let priv_packages = parse_package_csv(&priv_app)?;

            validate_package_sets(&system_packages, &priv_packages)?;

            config.app.system_app = system_packages;
            config.app.priv_app = priv_packages;
            config.save()?;
            println!("replaced package configuration");
        }
    }

    Ok(())
}

fn ensure_not_in_other_group(
    existing_packages: &BTreeSet<String>,
    package: &str,
    group_name: &str,
) -> Result<()> {
    if existing_packages.contains(package) {
        bail!("Package `{package}` already exists in {group_name}");
    }

    Ok(())
}

fn parse_package_csv(input: &str) -> Result<BTreeSet<String>> {
    let mut packages = BTreeSet::new();

    for package in input
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    {
        validate_package_name(package)?;
        packages.insert(package.to_owned());
    }

    Ok(packages)
}

fn validate_package_sets(
    system_packages: &BTreeSet<String>,
    priv_packages: &BTreeSet<String>,
) -> Result<()> {
    let duplicates: Vec<_> = system_packages
        .intersection(priv_packages)
        .cloned()
        .collect();

    if !duplicates.is_empty() {
        bail!(
            "Packages cannot exist in both system and priv groups: {}",
            duplicates.join(", ")
        );
    }

    Ok(())
}

fn list_installed_user_apps() -> Result<InstalledUserAppsListing> {
    match list_installed_user_apps_from_pm() {
        Ok(packages) => Ok(packages),
        Err(pm_error) => list_installed_user_apps_from_packages_xml().with_context(|| {
            format!(
                "Failed to list installed user apps for system user {SYSTEM_USER_ID} via `pm list packages -3 --user {SYSTEM_USER_ID}` and package metadata fallback: {pm_error}"
            )
        }),
    }
}

fn list_installed_user_apps_from_pm() -> Result<InstalledUserAppsListing> {
    let output = Command::new("pm")
        .args(["list", "packages", "-3", "--user", SYSTEM_USER_ID])
        .output()
        .with_context(|| {
            format!("Failed to execute `pm list packages -3 --user {SYSTEM_USER_ID}`")
        })?;

    if !output.status.success() {
        bail!(
            "`pm list packages -3 --user {SYSTEM_USER_ID}` failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(InstalledUserAppsListing {
        packages: parse_pm_list_output(&stdout),
        source: InstalledUserAppsSource::PmListPackages,
        system_user_state_source: None,
        warnings: Vec::new(),
    })
}

fn list_installed_user_apps_from_packages_xml() -> Result<InstalledUserAppsListing> {
    let packages = list_known_user_apps_from_packages_xml()?;
    match read_system_user_package_states() {
        Ok(Some(system_user_package_states)) => Ok(InstalledUserAppsListing {
            packages: filter_installed_for_system_user(packages, &system_user_package_states),
            source: InstalledUserAppsSource::PackagesXmlAndRestrictions,
            system_user_state_source: Some(SystemUserStateSource::PackageRestrictions),
            warnings: vec![format!(
                "Package discovery fell back to packages.xml plus package-restrictions metadata because `pm list packages -3 --user {SYSTEM_USER_ID}` was unavailable."
            )],
        }),
        Ok(None) => Ok(InstalledUserAppsListing {
            packages,
            source: InstalledUserAppsSource::PackagesXmlBestEffort,
            system_user_state_source: None,
            warnings: vec![format!(
                "No package-restrictions metadata was found for system user {SYSTEM_USER_ID}; returning a packages.xml best-effort set."
            )],
        }),
        Err(error) => Ok(InstalledUserAppsListing {
            packages,
            source: InstalledUserAppsSource::PackagesXmlBestEffort,
            system_user_state_source: None,
            warnings: vec![format!(
                "Failed to read package-restrictions metadata for system user {SYSTEM_USER_ID}: {error:#}; returning a packages.xml best-effort set."
            )],
        }),
    }
}

fn list_known_user_apps_from_packages_xml() -> Result<BTreeSet<String>> {
    let mut last_error = None;

    for packages_xml in PACKAGES_XML_PATHS {
        let path = Path::new(packages_xml);
        if !path.exists() {
            continue;
        }

        match read_installed_user_apps_from_packages_xml(path) {
            Ok(packages) => return Ok(packages),
            Err(error) => {
                last_error = Some(error);
            }
        }
    }

    match last_error {
        Some(error) => Err(error),
        None => Ok(BTreeSet::new()),
    }
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
                last_error = Some(error);
            }
        }
    }

    match last_error {
        Some(error) => Err(error),
        None => Ok(None),
    }
}

fn read_installed_user_apps_from_packages_xml(path: &Path) -> Result<BTreeSet<String>> {
    let contents =
        read_xmlish_text(path).with_context(|| format!("Failed to read {}", path.display()))?;
    parse_packages_xml_user_apps(&contents)
        .with_context(|| format!("Failed to parse {}", path.display()))
}

fn read_package_states_from_restrictions_file(
    path: &Path,
) -> Result<BTreeMap<String, SystemUserPackageState>> {
    let contents =
        read_xmlish_text(path).with_context(|| format!("Failed to read {}", path.display()))?;
    parse_package_restrictions_xml(&contents)
        .with_context(|| format!("Failed to parse {}", path.display()))
}

fn parse_pm_list_output(stdout: &str) -> BTreeSet<String> {
    stdout
        .lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix("package:"))
        .map(str::trim)
        .filter(|package| !package.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn parse_packages_xml_user_apps(contents: &str) -> Result<BTreeSet<String>> {
    let mut reader = Reader::from_str(contents);
    reader.config_mut().trim_text(true);
    let mut packages = BTreeSet::new();

    loop {
        match reader.read_event()? {
            Event::Start(event) | Event::Empty(event) if event.name().as_ref() == b"package" => {
                let mut package_name = None;
                let mut code_path = None;
                let mut public_flags = None;
                let mut legacy_system = None;

                for attribute in event.attributes().with_checks(false) {
                    let attribute = attribute?;
                    let value = attribute
                        .decode_and_unescape_value(reader.decoder())?
                        .into_owned();

                    match attribute.key.as_ref() {
                        b"name" => package_name = Some(value),
                        b"codePath" => code_path = Some(value),
                        b"publicFlags" | b"flags" => public_flags = parse_i64ish(&value),
                        b"system" => legacy_system = parse_boolish(&value),
                        _ => {}
                    }
                }

                if let (Some(package_name), Some(code_path)) = (package_name, code_path) {
                    if is_user_app(public_flags, legacy_system, &code_path) {
                        packages.insert(package_name);
                    }
                }
            }
            Event::Eof => return Ok(packages),
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

fn is_user_app(public_flags: Option<i64>, legacy_system: Option<bool>, code_path: &str) -> bool {
    if let Some(public_flags) = public_flags {
        return public_flags & APPLICATION_INFO_FLAG_SYSTEM == 0;
    }

    if let Some(legacy_system) = legacy_system {
        return !legacy_system;
    }

    has_known_user_app_prefix(Path::new(code_path))
}

fn filter_installed_for_system_user(
    packages: BTreeSet<String>,
    system_user_package_states: &BTreeMap<String, SystemUserPackageState>,
) -> BTreeSet<String> {
    packages
        .into_iter()
        .filter(|package| {
            system_user_package_states
                .get(package)
                .copied()
                .unwrap_or_default()
                .is_available()
        })
        .collect()
}

fn build_inspect_output(
    config: &App,
    installed_user_apps: &InstalledUserAppsListing,
) -> InspectOutput {
    let configured: BTreeSet<String> = config.system_app.union(&config.priv_app).cloned().collect();

    let missing_configured_apps = configured
        .difference(&installed_user_apps.packages)
        .cloned()
        .collect();
    let mut warnings = installed_user_apps.warnings.clone();
    let invalid_configured_packages = configured
        .iter()
        .filter(|package| !is_valid_package_name(package))
        .cloned()
        .collect::<Vec<_>>();
    if !invalid_configured_packages.is_empty() {
        warnings.push(format!(
            "Config contains invalid Android package names that will be ignored during apply: {}",
            invalid_configured_packages.join(", ")
        ));
    }

    InspectOutput {
        system_app: config.system_app.iter().cloned().collect(),
        priv_app: config.priv_app.iter().cloned().collect(),
        installed_user_apps: installed_user_apps.packages.iter().cloned().collect(),
        missing_configured_apps,
        installed_user_apps_source: installed_user_apps.source,
        system_user_state_source: installed_user_apps.system_user_state_source,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::{
        App, InspectOutput, InstalledUserAppsListing, InstalledUserAppsSource,
        SystemUserStateSource, build_inspect_output, filter_installed_for_system_user,
        parse_package_csv, parse_package_restrictions_xml, parse_packages_xml_user_apps,
        parse_pm_list_output, read_installed_user_apps_from_packages_xml,
        read_package_states_from_restrictions_file, validate_package_sets,
    };
    use crate::android_package_state::SystemUserPackageState;

    #[test]
    fn csv_parser_trims_entries_and_ignores_empty_values() {
        let parsed = parse_package_csv(" com.example.alpha ,,com.example.beta, ").unwrap();

        assert_eq!(
            parsed,
            BTreeSet::from([
                "com.example.alpha".to_string(),
                "com.example.beta".to_string(),
            ])
        );
    }

    #[test]
    fn csv_parser_rejects_invalid_android_package_names() {
        let error = parse_package_csv("com.example.valid,bad/package").unwrap_err();

        assert!(error.to_string().contains("bad/package"));
    }

    #[test]
    fn pm_list_parser_extracts_user_packages() {
        let parsed = parse_pm_list_output(
            "package:com.example.alpha\npackage:com.example.beta\nignored-line\npackage:\n",
        );

        assert_eq!(
            parsed,
            BTreeSet::from([
                "com.example.alpha".to_string(),
                "com.example.beta".to_string(),
            ])
        );
    }

    #[test]
    fn packages_xml_parser_extracts_non_system_packages() {
        let parsed = parse_packages_xml_user_apps(
            r#"
            <packages>
              <package name="com.example.user" codePath="/data/app/~~abc/com.example.user/base.apk" publicFlags="0" />
              <package name="com.example.adopted" codePath="/mnt/expand/uuid/app/com.example.adopted/base.apk" publicFlags="0" />
              <package name="com.example.system" codePath="/system/app/System/System.apk" publicFlags="1" />
              <package name="com.example.legacy" codePath="/data/app/legacy/base.apk" system="false" />
            </packages>
            "#,
        )
        .unwrap();

        assert_eq!(
            parsed,
            BTreeSet::from([
                "com.example.adopted".to_string(),
                "com.example.legacy".to_string(),
                "com.example.user".to_string(),
            ])
        );
    }

    #[test]
    fn packages_xml_parser_supports_legacy_flags_attribute() {
        let parsed = parse_packages_xml_user_apps(
            r#"
            <packages>
              <package name="com.example.user" codePath="/data/app/com.example.user/base.apk" flags="0" />
              <package name="com.example.system" codePath="/system/app/System/System.apk" flags="1" />
            </packages>
            "#,
        )
        .unwrap();

        assert_eq!(parsed, BTreeSet::from(["com.example.user".to_string()]));
    }

    #[test]
    fn packages_xml_parser_supports_legacy_data_app_private_paths() {
        let parsed = parse_packages_xml_user_apps(
            r#"
            <packages>
              <package name="com.example.locked" codePath="/data/app-private/com.example.locked/base.apk" />
              <package name="com.example.system" codePath="/system/app/System/System.apk" />
            </packages>
            "#,
        )
        .unwrap();

        assert_eq!(parsed, BTreeSet::from(["com.example.locked".to_string()]));
    }

    #[test]
    fn package_restrictions_parser_reads_system_user_install_states() {
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
    fn system_user_filter_excludes_packages_explicitly_uninstalled_for_user_zero() {
        let packages = BTreeSet::from([
            "com.example.alpha".to_string(),
            "com.example.beta".to_string(),
            "com.example.gamma".to_string(),
        ]);
        let system_user_states = BTreeMap::from([
            (
                "com.example.alpha".to_string(),
                SystemUserPackageState {
                    installed: true,
                    hidden: false,
                },
            ),
            (
                "com.example.beta".to_string(),
                SystemUserPackageState {
                    installed: false,
                    hidden: false,
                },
            ),
        ]);

        let filtered = filter_installed_for_system_user(packages, &system_user_states);

        assert_eq!(
            filtered,
            BTreeSet::from([
                "com.example.alpha".to_string(),
                "com.example.gamma".to_string(),
            ])
        );
    }

    #[test]
    fn system_user_filter_excludes_packages_hidden_for_user_zero() {
        let packages = BTreeSet::from([
            "com.example.alpha".to_string(),
            "com.example.beta".to_string(),
        ]);
        let system_user_states = BTreeMap::from([
            (
                "com.example.alpha".to_string(),
                SystemUserPackageState {
                    installed: true,
                    hidden: true,
                },
            ),
            (
                "com.example.beta".to_string(),
                SystemUserPackageState {
                    installed: true,
                    hidden: false,
                },
            ),
        ]);

        let filtered = filter_installed_for_system_user(packages, &system_user_states);

        assert_eq!(filtered, BTreeSet::from(["com.example.beta".to_string()]));
    }

    #[test]
    fn packages_xml_reader_keeps_current_empty_state_instead_of_falling_back_to_stale_backup() {
        let dir = tempfile::tempdir().unwrap();
        let current = dir.path().join("packages.xml");
        let backup = dir.path().join("packages-backup.xml");

        std::fs::write(&current, "<packages></packages>").unwrap();
        std::fs::write(
            &backup,
            r#"<packages><package name="com.example.stale" codePath="/data/app/stale/base.apk" publicFlags="0" /></packages>"#,
        )
        .unwrap();

        let current_packages = read_installed_user_apps_from_packages_xml(&current).unwrap();
        let backup_packages = read_installed_user_apps_from_packages_xml(&backup).unwrap();

        assert!(current_packages.is_empty());
        assert_eq!(
            backup_packages,
            BTreeSet::from(["com.example.stale".to_string()])
        );
    }

    #[test]
    fn packages_xml_reader_rejects_invalid_current_file() {
        let dir = tempfile::tempdir().unwrap();
        let current = dir.path().join("packages.xml");

        std::fs::write(&current, "<packages><package").unwrap();

        assert!(read_installed_user_apps_from_packages_xml(&current).is_err());
    }

    #[test]
    fn packages_xml_reader_supports_android_binary_xml() {
        let dir = tempfile::tempdir().unwrap();
        let packages_xml = dir.path().join("packages.xml");

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
        abx.extend_from_slice(&(16_u16).to_be_bytes());
        abx.extend_from_slice(b"com.example.user");

        abx.push(0x2F);
        abx.extend_from_slice(&0xFFFF_u16.to_be_bytes());
        abx.extend_from_slice(&(8_u16).to_be_bytes());
        abx.extend_from_slice(b"codePath");
        let code_path = "/data/app/~~abc/com.example.user/base.apk";
        abx.extend_from_slice(&(code_path.len() as u16).to_be_bytes());
        abx.extend_from_slice(code_path.as_bytes());

        abx.push(0x7F);
        abx.extend_from_slice(&0xFFFF_u16.to_be_bytes());
        abx.extend_from_slice(&(11_u16).to_be_bytes());
        abx.extend_from_slice(b"publicFlags");
        abx.extend_from_slice(&(0_i32).to_be_bytes());

        abx.push(0x03);
        abx.extend_from_slice(&1_u16.to_be_bytes());
        abx.push(0x03);
        abx.extend_from_slice(&0_u16.to_be_bytes());
        abx.push(0x01);

        std::fs::write(&packages_xml, abx).unwrap();

        let packages = read_installed_user_apps_from_packages_xml(&packages_xml).unwrap();

        assert_eq!(packages, BTreeSet::from(["com.example.user".to_string()]));
    }

    #[test]
    fn package_restrictions_reader_supports_android_binary_xml() {
        let dir = tempfile::tempdir().unwrap();
        let restrictions = dir.path().join("package-restrictions.xml");

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
        abx.extend_from_slice(&(16_u16).to_be_bytes());
        abx.extend_from_slice(b"com.example.user");

        abx.push(0xCF);
        abx.extend_from_slice(&0xFFFF_u16.to_be_bytes());
        abx.extend_from_slice(&(4_u16).to_be_bytes());
        abx.extend_from_slice(b"inst");

        abx.push(0x03);
        abx.extend_from_slice(&1_u16.to_be_bytes());
        abx.push(0x03);
        abx.extend_from_slice(&0_u16.to_be_bytes());
        abx.push(0x01);

        std::fs::write(&restrictions, abx).unwrap();

        let states = read_package_states_from_restrictions_file(&restrictions).unwrap();

        assert_eq!(
            states,
            BTreeMap::from([(
                "com.example.user".to_string(),
                SystemUserPackageState {
                    installed: true,
                    hidden: false,
                }
            )])
        );
    }

    #[test]
    fn validate_package_sets_rejects_duplicate_membership() {
        let error = validate_package_sets(
            &BTreeSet::from(["com.example.shared".to_string()]),
            &BTreeSet::from(["com.example.shared".to_string()]),
        )
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("Packages cannot exist in both system and priv groups")
        );
    }

    #[test]
    fn inspect_output_reports_missing_packages() {
        let app = App {
            system_app: BTreeSet::from([
                "com.example.alpha".to_string(),
                "com.example.beta".to_string(),
            ]),
            priv_app: BTreeSet::from(["com.example.gamma".to_string()]),
        };
        let installed = BTreeSet::from([
            "com.example.alpha".to_string(),
            "com.example.delta".to_string(),
        ]);
        let installed_listing = InstalledUserAppsListing {
            packages: installed,
            source: InstalledUserAppsSource::PmListPackages,
            system_user_state_source: None,
            warnings: Vec::new(),
        };

        let output = build_inspect_output(&app, &installed_listing);

        assert_eq!(
            output,
            InspectOutput {
                system_app: vec![
                    "com.example.alpha".to_string(),
                    "com.example.beta".to_string(),
                ],
                priv_app: vec!["com.example.gamma".to_string()],
                installed_user_apps: vec![
                    "com.example.alpha".to_string(),
                    "com.example.delta".to_string(),
                ],
                missing_configured_apps: vec![
                    "com.example.beta".to_string(),
                    "com.example.gamma".to_string(),
                ],
                installed_user_apps_source: InstalledUserAppsSource::PmListPackages,
                system_user_state_source: None,
                warnings: Vec::new(),
            }
        );
    }

    #[test]
    fn inspect_output_preserves_metadata_fallback_details() {
        let app = App {
            system_app: BTreeSet::from(["com.example.alpha".to_string()]),
            priv_app: BTreeSet::new(),
        };
        let installed_listing = InstalledUserAppsListing {
            packages: BTreeSet::from(["com.example.alpha".to_string()]),
            source: InstalledUserAppsSource::PackagesXmlAndRestrictions,
            system_user_state_source: Some(SystemUserStateSource::PackageRestrictions),
            warnings: vec!["fallback".to_string()],
        };

        let output = build_inspect_output(&app, &installed_listing);

        assert_eq!(
            output,
            InspectOutput {
                system_app: vec!["com.example.alpha".to_string()],
                priv_app: Vec::new(),
                installed_user_apps: vec!["com.example.alpha".to_string()],
                missing_configured_apps: Vec::new(),
                installed_user_apps_source: InstalledUserAppsSource::PackagesXmlAndRestrictions,
                system_user_state_source: Some(SystemUserStateSource::PackageRestrictions),
                warnings: vec!["fallback".to_string()],
            }
        );
    }

    #[test]
    fn inspect_output_warns_about_invalid_configured_package_names() {
        let app = App {
            system_app: BTreeSet::from(["../escape".to_string()]),
            priv_app: BTreeSet::new(),
        };
        let installed_listing = InstalledUserAppsListing {
            packages: BTreeSet::new(),
            source: InstalledUserAppsSource::PmListPackages,
            system_user_state_source: None,
            warnings: Vec::new(),
        };

        let output = build_inspect_output(&app, &installed_listing);

        assert_eq!(output.warnings.len(), 1);
        assert!(output.warnings[0].contains("../escape"));
    }
}
