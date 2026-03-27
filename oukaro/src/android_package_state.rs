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
