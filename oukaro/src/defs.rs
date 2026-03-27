pub const CONFIG_PATH: &str = "/data/adb/modules/oukaro_manager/config.toml";
pub const WORK_PATH: &str = "/data/adb/modules/oukaro_manager/work";
pub const LOWER_PATH: &str = "/system";
pub const UPPER_PATH: &str = "/data/adb/modules/oukaro_manager/system";
pub const SYSTEM_USER_ID: &str = "0";
pub const PACKAGES_XML_PATHS: &[&str] = &[
    "/data/system/packages.xml",
    "/data/system/packages.xml.reservecopy",
    "/data/system/packages-backup.xml",
];
pub const SYSTEM_USER_PACKAGE_RESTRICTIONS_PATHS: &[&str] = &[
    "/data/system/users/0/package-restrictions.xml",
    "/data/system/users/0/package-restrictions.xml.reservecopy",
    "/data/system/users/0/package-restrictions-backup.xml",
    "/data/system/users/0/package-restrictions.xml.bak",
];
