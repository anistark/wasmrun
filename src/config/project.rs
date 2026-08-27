//! Project-level `wasmrun.toml`: configuration that sits next to the code it
//! describes, as opposed to the global `~/.wasmrun/config.toml`.
//!
//! Only `[os.network]` is read today. Unknown tables are ignored on purpose so
//! the file can grow (`[os]`, `[os.permissions]`) without every older wasmrun
//! rejecting a project it could otherwise run.

use crate::error::{ConfigError, Result};
use serde::Deserialize;
use std::net::IpAddr;
use std::path::Path;
use wasmnet::policy::{NetworkPolicy, PolicyConfig};

/// The file wasmrun looks for at the project root.
pub const PROJECT_CONFIG_FILE: &str = "wasmrun.toml";

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProjectConfig {
    #[serde(default)]
    pub os: OsConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct OsConfig {
    pub network: Option<NetworkConfig>,
}

/// The `[os.network]` table, mapped onto wasmnet's policy.
///
/// Every field is optional and an absent one keeps wasmnet's default, so a
/// table that sets only `allow` still denies RFC1918. Unknown keys are an
/// error: a misspelled `max_connection` that silently did nothing would be a
/// policy the user thinks they have and does not.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkConfig {
    pub allow: Option<Vec<String>>,
    pub deny: Option<Vec<String>>,
    pub bind_ports: Option<String>,
    pub max_connections: Option<usize>,
    pub max_bandwidth_mbps: Option<u32>,
    pub connection_timeout_secs: Option<u64>,
}

impl ProjectConfig {
    /// Read `wasmrun.toml` from a project directory.
    ///
    /// A missing file is the common case and gives the defaults. A file that
    /// exists and does not parse is an error: it was written to be obeyed, and
    /// quietly running under different settings than the ones on disk is worse
    /// than refusing.
    pub fn load(project_dir: impl AsRef<Path>) -> Result<Self> {
        let path = project_dir.as_ref().join(PROJECT_CONFIG_FILE);
        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = std::fs::read_to_string(&path).map_err(|e| ConfigError::ParseError {
            message: format!("could not read {}: {e}", path.display()),
        })?;

        toml::from_str(&contents).map_err(|e| {
            ConfigError::ParseError {
                message: format!("{}: {e}", path.display()),
            }
            .into()
        })
    }

    /// The network policy for this project, defaults included.
    pub fn network_policy(&self) -> Result<PolicyConfig> {
        match &self.os.network {
            Some(network) => network.to_policy_config(),
            None => Ok(PolicyConfig::default()),
        }
    }

    /// Whether the project asked for a policy of its own, which is worth
    /// saying out loud when the proxy starts.
    pub fn has_network_config(&self) -> bool {
        self.os.network.is_some()
    }
}

impl NetworkConfig {
    /// Fold the configured fields into wasmnet's default policy.
    pub fn to_policy_config(&self) -> Result<PolicyConfig> {
        self.to_policy_config_named("os.network")
    }

    /// The same, naming the table in any error. The `[os.network]` table and a
    /// tenant's `[tenants.network]` table have the same shape, and a message
    /// that names the wrong one sends the reader to the wrong file.
    pub fn to_policy_config_named(&self, table: &str) -> Result<PolicyConfig> {
        let mut network = NetworkPolicy::default();

        if let Some(allow) = &self.allow {
            validate_rules(table, "allow", allow)?;
            network.allow = allow.clone();
        }
        if let Some(deny) = &self.deny {
            validate_rules(table, "deny", deny)?;
            network.deny = deny.clone();
        }
        if let Some(bind_ports) = &self.bind_ports {
            validate_bind_ports(table, bind_ports)?;
            network.bind_ports = bind_ports.clone();
        }
        if let Some(max_connections) = self.max_connections {
            network.max_connections = max_connections;
        }
        if let Some(max_bandwidth_mbps) = self.max_bandwidth_mbps {
            network.max_bandwidth_mbps = max_bandwidth_mbps;
        }
        if let Some(connection_timeout_secs) = self.connection_timeout_secs {
            network.connection_timeout_secs = connection_timeout_secs;
        }

        Ok(PolicyConfig { network })
    }
}

/// A policy the user wrote that wasmrun will not run under.
fn invalid(message: impl Into<String>) -> crate::error::WasmrunError {
    ConfigError::InvalidValue {
        message: message.into(),
    }
    .into()
}

/// Check every allow/deny rule before wasmnet sees it.
///
/// wasmnet parses a rule as a CIDR first and falls back to treating it as a
/// domain name, and its domain parser accepts nearly anything. So `10.0.0/8`
/// becomes a *hostname* that no address ever matches, and a `deny` written
/// that way is a rule the user believes in and does not have. Both mistakes
/// are caught here, where there is a file and a line to point at.
fn validate_rules(table: &str, field: &str, rules: &[String]) -> Result<()> {
    for rule in rules {
        if rule == "*" {
            continue;
        }

        if rule.trim().is_empty() {
            return Err(invalid(format!(
                "[{table}] {field}: an empty rule matches nothing; remove it or use \"*\""
            )));
        }

        if let Some((addr, prefix)) = rule.split_once('/') {
            validate_cidr(table, field, rule, addr, prefix)?;
            continue;
        }

        // wasmnet strips a `:port` suffix the same way before deciding what a
        // rule is, so validation has to look at the same host it will.
        let host = split_host(rule);
        if host.parse::<IpAddr>().is_ok() {
            return Err(invalid(format!(
                "[{table}] {field}: \"{rule}\" is a bare IP address, which is matched as a \
                 hostname and never matches a connection. Write it as a CIDR range instead, \
                 for example \"{host}/32\" (\"/128\" for IPv6)"
            )));
        }

        if host.contains(char::is_whitespace) {
            return Err(invalid(format!(
                "[{table}] {field}: \"{rule}\" is not a valid host pattern"
            )));
        }
    }

    Ok(())
}

fn validate_cidr(table: &str, field: &str, rule: &str, addr: &str, prefix: &str) -> Result<()> {
    let ip: IpAddr = addr.parse().map_err(|_| {
        invalid(format!(
            "[{table}] {field}: \"{rule}\" looks like a CIDR range but \"{addr}\" is not an \
             IP address"
        ))
    })?;

    let bits: u8 = prefix.parse().map_err(|_| {
        invalid(format!(
            "[{table}] {field}: \"{rule}\" has a prefix length that is not a number"
        ))
    })?;

    let max = if ip.is_ipv4() { 32 } else { 128 };
    if bits > max {
        return Err(invalid(format!(
            "[{table}] {field}: \"{rule}\" has a prefix length above /{max}"
        )));
    }

    Ok(())
}

/// A rule's host, with any `:port` suffix removed, matching how wasmnet splits
/// one. Note that it splits on the last colon, so an unbracketed IPv6 address
/// loses its final group; that is wasmnet's rule and validation has to see the
/// rule the same way it will.
fn split_host(rule: &str) -> &str {
    match rule.rsplit_once(':') {
        Some((host, port)) if port.parse::<u16>().is_ok() => host,
        _ => rule,
    }
}

/// Check a `bind_ports` string. wasmnet drops the parts it cannot read, so an
/// unreadable string leaves a policy that binds nothing at all, silently.
fn validate_bind_ports(table: &str, spec: &str) -> Result<()> {
    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return Err(invalid(format!(
                "[{table}] bind_ports: empty range in the list"
            )));
        }

        let bad = |what: &str| {
            invalid(format!(
                "[{table}] bind_ports: \"{part}\" {what}. Use a port or a range, \
                 for example \"3000-9999\" or \"8080,9000-9100\""
            ))
        };

        match part.split_once('-') {
            Some((low, high)) => {
                let low: u16 = low.trim().parse().map_err(|_| bad("is not a port range"))?;
                let high: u16 = high
                    .trim()
                    .parse()
                    .map_err(|_| bad("is not a port range"))?;
                if low > high {
                    return Err(bad("ends below where it starts"));
                }
            }
            None => {
                part.parse::<u16>().map_err(|_| bad("is not a port"))?;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_config(contents: &str) -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(PROJECT_CONFIG_FILE), contents).unwrap();
        dir
    }

    #[test]
    fn a_missing_file_gives_the_safe_default() {
        let dir = TempDir::new().unwrap();
        let config = ProjectConfig::load(dir.path()).unwrap();

        assert!(!config.has_network_config());
        let policy = config.network_policy().unwrap().network;
        assert!(policy.deny.contains(&"10.0.0.0/8".to_string()));
        assert!(policy.deny.contains(&"127.0.0.0/8".to_string()));
        assert!(policy.deny.contains(&"169.254.0.0/16".to_string()));
    }

    #[test]
    fn an_unset_table_still_denies_private_ranges() {
        let dir = write_config("[os.network]\nallow = [\"api.example.com:443\"]\n");
        let config = ProjectConfig::load(dir.path()).unwrap();

        assert!(config.has_network_config());
        let policy = config.network_policy().unwrap().network;
        assert_eq!(policy.allow, vec!["api.example.com:443".to_string()]);
        // Setting `allow` must not drop the default deny list.
        assert!(policy.deny.contains(&"192.168.0.0/16".to_string()));
        assert_eq!(policy.bind_ports, "3000-9999");
    }

    #[test]
    fn every_field_maps_through() {
        let dir = write_config(
            r#"
[os.network]
allow = ["*.github.com:443", "203.0.113.0/24"]
deny = ["10.0.0.0/8"]
bind_ports = "8080,9000-9100"
max_connections = 5
max_bandwidth_mbps = 2
connection_timeout_secs = 15
"#,
        );

        let policy = ProjectConfig::load(dir.path())
            .unwrap()
            .network_policy()
            .unwrap()
            .network;

        assert_eq!(policy.allow.len(), 2);
        assert_eq!(policy.deny, vec!["10.0.0.0/8".to_string()]);
        assert_eq!(policy.bind_ports, "8080,9000-9100");
        assert_eq!(policy.max_connections, 5);
        assert_eq!(policy.max_bandwidth_mbps, 2);
        assert_eq!(policy.connection_timeout_secs, 15);
    }

    #[test]
    fn other_tables_are_ignored() {
        let dir = write_config(
            "[os]\nlanguage = \"nodejs\"\n\n[os.permissions]\nnetwork = true\n\n[plugins]\nx = 1\n",
        );

        let config = ProjectConfig::load(dir.path()).unwrap();
        assert!(!config.has_network_config());
    }

    #[test]
    fn a_malformed_file_is_an_error() {
        let dir = write_config("[os.network\nallow = [\n");
        let err = ProjectConfig::load(dir.path()).unwrap_err().to_string();
        assert!(err.contains("wasmrun.toml"), "{err}");
    }

    #[test]
    fn an_unknown_key_is_an_error() {
        let dir = write_config("[os.network]\nmax_connection = 5\n");
        let err = ProjectConfig::load(dir.path()).unwrap_err().to_string();
        assert!(err.contains("max_connection"), "{err}");
    }

    #[test]
    fn a_malformed_cidr_is_rejected() {
        let dir = write_config("[os.network]\ndeny = [\"10.0.0/8\"]\n");
        let err = ProjectConfig::load(dir.path())
            .unwrap()
            .network_policy()
            .unwrap_err()
            .to_string();
        assert!(err.contains("not an IP address"), "{err}");
    }

    #[test]
    fn an_oversized_prefix_is_rejected() {
        let dir = write_config("[os.network]\nallow = [\"192.0.2.0/33\"]\n");
        let err = ProjectConfig::load(dir.path())
            .unwrap()
            .network_policy()
            .unwrap_err()
            .to_string();
        assert!(err.contains("above /32"), "{err}");
    }

    #[test]
    fn a_bare_ip_is_rejected_with_the_cidr_it_should_be() {
        let dir = write_config("[os.network]\nallow = [\"203.0.113.7\"]\n");
        let err = ProjectConfig::load(dir.path())
            .unwrap()
            .network_policy()
            .unwrap_err()
            .to_string();
        assert!(err.contains("203.0.113.7/32"), "{err}");
    }

    #[test]
    fn a_bare_ip_with_a_port_is_rejected_too() {
        let dir = write_config("[os.network]\nallow = [\"203.0.113.7:443\"]\n");
        let err = ProjectConfig::load(dir.path())
            .unwrap()
            .network_policy()
            .unwrap_err()
            .to_string();
        assert!(err.contains("bare IP address"), "{err}");
    }

    #[test]
    fn a_domain_with_a_port_is_fine() {
        let dir = write_config("[os.network]\nallow = [\"api.example.com:443\", \"*\"]\n");
        assert!(ProjectConfig::load(dir.path())
            .unwrap()
            .network_policy()
            .is_ok());
    }

    /// The mapping is only worth anything if wasmnet's own policy engine reads
    /// it the way the file meant, so these go through `Policy` rather than
    /// asserting on the struct we just filled in.
    mod enforcement {
        use super::*;
        use wasmnet::policy::Policy;

        fn policy_for(contents: &str) -> Policy {
            let dir = write_config(contents);
            let config = ProjectConfig::load(dir.path()).unwrap();
            Policy::new(&config.network_policy().unwrap().network)
        }

        #[test]
        fn the_default_blocks_private_ranges_and_allows_the_rest() {
            let policy = policy_for("[os]\nlanguage = \"nodejs\"\n");

            assert!(policy.check_connect("10.1.2.3", 80).is_err());
            assert!(policy.check_connect("127.0.0.1", 80).is_err());
            assert!(policy.check_connect("169.254.169.254", 80).is_err());
            assert!(policy.check_connect("example.com", 443).is_ok());
        }

        #[test]
        fn an_allow_list_keeps_the_default_deny_underneath_it() {
            let policy = policy_for("[os.network]\nallow = [\"*.github.com:443\"]\n");

            assert!(policy.check_connect("api.github.com", 443).is_ok());
            assert!(policy.check_connect("example.com", 443).is_err());
            // Not listed in `deny`, but the default deny list is still there.
            assert!(policy.check_connect("192.168.1.1", 443).is_err());
        }

        #[test]
        fn an_allowed_cidr_reaches_the_ip_path() {
            let policy = policy_for(
                "[os.network]\nallow = [\"203.0.113.0/24\"]\ndeny = [\"203.0.113.9/32\"]\n",
            );

            assert!(policy.check_connect("203.0.113.7", 80).is_ok());
            assert!(policy.check_connect("203.0.113.9", 80).is_err());
            assert!(policy.check_connect("198.51.100.1", 80).is_err());
        }

        #[test]
        fn bind_ports_bound_what_the_vm_may_listen_on() {
            let policy = policy_for("[os.network]\nbind_ports = \"3000-3999\"\n");

            assert!(policy.check_bind(3500).is_ok());
            assert!(policy.check_bind(8080).is_err());
        }
    }

    #[test]
    fn bad_bind_ports_are_rejected() {
        for spec in ["http", "9000-", "9100-9000", "3000,,4000"] {
            let dir = write_config(&format!("[os.network]\nbind_ports = \"{spec}\"\n"));
            assert!(
                ProjectConfig::load(dir.path())
                    .unwrap()
                    .network_policy()
                    .is_err(),
                "expected {spec:?} to be rejected"
            );
        }
    }

    #[test]
    fn good_bind_ports_are_accepted() {
        for spec in ["3000-9999", "8080", "8080,9000-9100", " 8080 , 9000 "] {
            let dir = write_config(&format!("[os.network]\nbind_ports = \"{spec}\"\n"));
            assert!(
                ProjectConfig::load(dir.path())
                    .unwrap()
                    .network_policy()
                    .is_ok(),
                "expected {spec:?} to be accepted"
            );
        }
    }
}
