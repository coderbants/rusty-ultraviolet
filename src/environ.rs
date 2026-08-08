//! Cleanroom Rust port of upstream Go source file: `environ.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! <public-docs>
//! The process environment as an ordered list of `KEY=VALUE` entries.
//! </public-docs>

/// Environ is a slice of strings that represents the environment variables of
/// the program.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Environ(pub Vec<String>);

impl Environ {
    /// Getenv returns the value of the environment variable named by the key.
    /// If the variable is not present in the environment, the value returned
    /// will be the empty string.
    pub fn getenv(&self, key: &str) -> String {
        self.lookup_env(key).unwrap_or_default()
    }

    /// LookupEnv retrieves the value of the environment variable named by the
    /// key. If the variable is present in the environment the value (which
    /// may be empty) is returned; otherwise None.
    pub fn lookup_env(&self, key: &str) -> Option<String> {
        let prefix = format!("{key}=");
        self.0
            .iter()
            .rev()
            .find(|entry| entry.starts_with(&prefix))
            .map(|entry| entry[prefix.len()..].to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environ() {
        let e = Environ(vec![
            "TERM=xterm".to_string(),
            "FOO=bar".to_string(),
            "TERM=screen".to_string(),
        ]);
        assert_eq!(e.getenv("TERM"), "screen");
        assert_eq!(e.getenv("FOO"), "bar");
        assert_eq!(e.getenv("MISSING"), "");
        assert_eq!(e.lookup_env("MISSING"), None);
        assert_eq!(e.lookup_env("FOO"), Some("bar".to_string()));
    }
}
