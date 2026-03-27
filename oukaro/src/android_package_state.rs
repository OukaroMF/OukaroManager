#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemUserPackageState {
    pub installed: bool,
    pub hidden: bool,
}

impl Default for SystemUserPackageState {
    fn default() -> Self {
        Self {
            installed: true,
            hidden: false,
        }
    }
}

impl SystemUserPackageState {
    pub fn is_available(self) -> bool {
        self.installed && !self.hidden
    }
}

#[cfg(test)]
mod tests {
    use super::SystemUserPackageState;

    #[test]
    fn package_state_matches_android_is_available_semantics() {
        assert!(SystemUserPackageState::default().is_available());
        assert!(
            !SystemUserPackageState {
                installed: false,
                hidden: false,
            }
            .is_available()
        );
        assert!(
            !SystemUserPackageState {
                installed: true,
                hidden: true,
            }
            .is_available()
        );
    }
}
