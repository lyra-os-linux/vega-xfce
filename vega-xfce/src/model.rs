// RPM builds inject the package release through VEGA_VERSION so the version
// shown by the application cannot drift from the package installed by the
// user. Direct Cargo builds fall back to the manifest version.
pub const APPLICATION_VERSION: &str = match option_env!("VEGA_VERSION") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppIdentity {
    pub name: String,
    pub version: String,
}

impl Default for AppIdentity {
    fn default() -> Self {
        Self {
            name: "Vega".into(),
            version: APPLICATION_VERSION.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{APPLICATION_VERSION, AppIdentity};

    #[test]
    fn identity_comes_from_package_metadata() {
        let identity = AppIdentity::default();
        assert_eq!(identity.name, "Vega");
        assert_eq!(identity.version, APPLICATION_VERSION);
    }
}
