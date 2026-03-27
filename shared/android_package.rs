use anyhow::{Result, bail};

pub fn is_valid_package_name(package: &str) -> bool {
    validate_package_name(package).is_ok()
}

pub fn validate_package_name(package: &str) -> Result<()> {
    if package.is_empty() {
        bail!("Package name must not be empty");
    }

    let segments = package.split('.').collect::<Vec<_>>();
    if segments.len() < 2 {
        bail!("Package name `{package}` must contain at least two segments");
    }

    for segment in segments {
        if segment.is_empty() {
            bail!("Package name `{package}` must not contain empty segments");
        }

        let mut chars = segment.chars();
        let first = chars.next().expect("segment is non-empty");
        if !first.is_ascii_alphabetic() {
            bail!(
                "Package name `{package}` has invalid segment `{segment}`: each segment must start with an ASCII letter"
            );
        }

        if chars.any(|ch| !ch.is_ascii_alphanumeric() && ch != '_') {
            bail!(
                "Package name `{package}` has invalid segment `{segment}`: only ASCII letters, digits, and underscores are allowed"
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{is_valid_package_name, validate_package_name};

    #[test]
    fn android_package_rules_accept_standard_application_ids() {
        assert!(is_valid_package_name("com.example.app"));
        assert!(is_valid_package_name("com.Example_1.app_2"));
    }

    #[test]
    fn android_package_rules_reject_invalid_names() {
        assert!(validate_package_name("").is_err());
        assert!(validate_package_name("single").is_err());
        assert!(validate_package_name("com..example").is_err());
        assert!(validate_package_name("1com.example").is_err());
        assert!(validate_package_name("com.example-app").is_err());
        assert!(validate_package_name("com.example/app").is_err());
    }
}
