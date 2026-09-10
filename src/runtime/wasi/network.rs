//! What a sandboxed program is allowed to connect to.
//!
//! Policy lives on the `WasiEnv`, one per execution, so the source of it is
//! the caller's business: `wasmrun exec` builds one from its flags, and the
//! agent builds a different one per tenant. The syscalls only ever see the
//! resolved answer.
//!
//! **Nothing is allowed unless it was configured.** wasmnet's own default
//! (deny the private ranges, allow the rest of the internet) is the right
//! default for a developer running their own project on their own machine, and
//! the wrong one for a server running code it was handed. Egress is something
//! an operator turns on.

use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;
use wasmnet::policy::{Policy, PolicyConfig};

/// Why a connection was refused, in the guest's terms.
#[derive(Debug)]
pub struct Denied {
    /// What to report to the program.
    pub errno: i32,
    /// What to tell a human, if anyone is reading logs.
    pub reason: String,
}

/// The network a program may reach.
#[derive(Clone, Default)]
pub struct NetworkAccess {
    /// `None` means no network at all, which is the default.
    policy: Option<Arc<Policy>>,
}

impl NetworkAccess {
    /// No network. Every connection attempt is refused.
    pub fn denied() -> Self {
        Self { policy: None }
    }

    /// The network described by a policy.
    pub fn with_policy(config: &PolicyConfig) -> Self {
        Self {
            policy: Some(Arc::new(Policy::new(&config.network))),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.policy.is_some()
    }

    /// How long a connect attempt may take, from the policy that allowed it.
    pub fn connect_timeout(&self) -> Duration {
        self.policy
            .as_ref()
            .map(|p| Duration::from_secs(p.connection_timeout_secs))
            .unwrap_or(Duration::from_secs(10))
    }

    /// Decide where a connection may go, resolving the name here so the
    /// policy sees the addresses the connection will actually use.
    ///
    /// This is the ordering that matters. A policy checked only against the
    /// string the guest supplied is a policy any guest can talk its way
    /// around, because a name it controls can resolve into a range the deny
    /// list covers: `localhost` and a cloud metadata endpoint both pass a
    /// check that only understands literal addresses. So the host is checked
    /// first (that is what domain rules are for), then resolved, then every
    /// resolved address is checked, and the caller connects to one of the
    /// addresses that passed rather than re-resolving the name and possibly
    /// getting a different answer.
    ///
    /// wasmnet 0.2.0 splits the decision the same way and names the halves:
    /// `check_connect` rules on the string, `check_resolved` on each address
    /// it resolved to. That is [wasmnet#3](https://github.com/anistark/wasmnet/issues/3),
    /// fixed upstream. The loop stays here regardless, because wasmrun does
    /// its own connecting and the proxy's ordering is not in the path: what
    /// this function owns is connecting to an address that passed rather than
    /// re-resolving the name and possibly getting a different one.
    pub fn resolve_and_check(&self, host: &str, port: u16) -> Result<Vec<SocketAddr>, Denied> {
        let policy = match &self.policy {
            Some(policy) => policy,
            None => {
                return Err(Denied {
                    errno: crate::runtime::wasi::syscalls::WASI_EACCES,
                    reason: "no network policy is configured, so the sandbox has no network"
                        .to_string(),
                })
            }
        };

        // A literal address needs no name check and no resolution.
        if let Ok(ip) = host.parse::<IpAddr>() {
            policy.check_connect(host, port).map_err(|reason| Denied {
                errno: crate::runtime::wasi::syscalls::WASI_EACCES,
                reason,
            })?;
            return Ok(vec![SocketAddr::new(ip, port)]);
        }

        // The name itself, so an allow list of domains means something and a
        // denied domain is refused before it is even looked up.
        policy.check_connect(host, port).map_err(|reason| Denied {
            errno: crate::runtime::wasi::syscalls::WASI_EACCES,
            reason,
        })?;

        let resolved = (host, port).to_socket_addrs().map_err(|e| Denied {
            errno: crate::runtime::wasi::syscalls::WASI_EIO,
            reason: format!("could not resolve {host}: {e}"),
        })?;

        // Every resolved address has to pass, not just one of them. A name
        // maps to a set the guest does not choose from, so "some of these are
        // allowed" means the connection may still land on one that is not.
        // `localhost` is the everyday case: it resolves to 127.0.0.1 and ::1,
        // and a deny list of IPv4 ranges (which is what wasmnet ships as its
        // default) covers the first and not the second, so accepting the
        // subset that passed would leave loopback reachable over IPv6.
        let mut allowed = Vec::new();
        for addr in resolved {
            if let Err(reason) = policy.check_resolved(addr.ip(), port) {
                return Err(Denied {
                    errno: crate::runtime::wasi::syscalls::WASI_EACCES,
                    reason: format!(
                        "{host} resolves to {}, which the policy refuses: {reason}",
                        addr.ip()
                    ),
                });
            }
            allowed.push(addr);
        }

        if allowed.is_empty() {
            return Err(Denied {
                errno: crate::runtime::wasi::syscalls::WASI_EIO,
                reason: format!("{host} resolved to no addresses"),
            });
        }
        Ok(allowed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasmnet::policy::NetworkPolicy;

    fn access(allow: &[&str], deny: &[&str]) -> NetworkAccess {
        let network = NetworkPolicy {
            allow: allow.iter().map(|s| s.to_string()).collect(),
            deny: deny.iter().map(|s| s.to_string()).collect(),
            ..NetworkPolicy::default()
        };
        NetworkAccess::with_policy(&PolicyConfig { network })
    }

    #[test]
    fn the_default_is_no_network_at_all() {
        let access = NetworkAccess::default();
        assert!(!access.is_enabled());
        let denied = access.resolve_and_check("example.com", 443).unwrap_err();
        assert_eq!(denied.errno, crate::runtime::wasi::syscalls::WASI_EACCES);
        assert!(denied.reason.contains("no network"), "{}", denied.reason);
    }

    #[test]
    fn a_literal_address_is_checked_as_itself() {
        let access = access(&["*"], &["10.0.0.0/8"]);
        assert!(access.resolve_and_check("10.1.2.3", 80).is_err());
        assert!(access.resolve_and_check("203.0.113.7", 80).is_ok());
    }

    #[test]
    fn a_name_that_resolves_into_a_denied_range_is_refused() {
        // The bug this ordering exists to prevent: `localhost` is a name, so a
        // check that only understands literal addresses lets it through, and
        // the connection then lands on 127.0.0.1 which the policy denies.
        let access = access(&["*"], &["127.0.0.0/8"]);
        let denied = access.resolve_and_check("localhost", 80).unwrap_err();
        assert!(
            denied.reason.contains("the policy refuses"),
            "{}",
            denied.reason
        );
    }

    #[test]
    fn one_refused_address_refuses_the_whole_name() {
        // `localhost` resolves to 127.0.0.1 and ::1 on most machines, and this
        // deny list only covers the first. Taking the subset that passed would
        // hand the guest loopback over IPv6.
        let v4_only = access(&["*"], &["127.0.0.0/8"]);
        assert!(v4_only.resolve_and_check("localhost", 80).is_err());

        // Denying both families refuses it either way.
        let both = access(&["*"], &["127.0.0.0/8", "::1/128"]);
        assert!(both.resolve_and_check("localhost", 80).is_err());
    }

    #[test]
    fn a_denied_domain_is_refused_before_it_is_resolved() {
        let access = access(&["*"], &["localhost"]);
        let denied = access.resolve_and_check("localhost", 80).unwrap_err();
        assert!(
            denied.reason.contains("blocked by policy"),
            "{}",
            denied.reason
        );
    }

    #[test]
    fn a_domain_allow_rule_survives_resolution() {
        // The allow decision is made on the name. Checking the allow list
        // again against 127.0.0.1 would refuse a rule that named `localhost`,
        // since an address never matches a domain pattern.
        let access = access(&["localhost"], &[]);
        assert!(access.resolve_and_check("localhost", 8080).is_ok());
    }

    #[test]
    fn an_allow_rule_covers_subdomains() {
        // wasmnet 0.2.0 made a bare domain rule cover its subdomains, where it
        // used to match exactly. `--allow-net "example.com"` is now the same
        // grant as `*.example.com`, so a rule meant to name one host has to
        // carry a port to narrow it.
        let access = access(&["example.com"], &[]);
        assert!(access
            .policy
            .as_ref()
            .unwrap()
            .check_connect("api.example.com", 443)
            .is_ok());
        assert!(access
            .policy
            .as_ref()
            .unwrap()
            .check_connect("example.com.evil.com", 443)
            .is_err());
    }

    #[test]
    fn a_port_in_a_rule_is_enforced() {
        // Also new in 0.2.0: a rule's `:port` used to be advisory on the
        // connect path, so `api.example.com:443` opened every port on it.
        let access = access(&["example.com:443"], &[]);
        let policy = access.policy.as_ref().unwrap();
        assert!(policy.check_connect("example.com", 443).is_ok());
        assert!(policy.check_connect("example.com", 8080).is_err());
    }

    #[test]
    fn a_name_outside_the_allow_list_is_refused() {
        let access = access(&["*.github.com:443"], &[]);
        assert!(access.resolve_and_check("example.com", 443).is_err());
    }

    #[test]
    fn an_allowed_name_comes_back_as_addresses_to_connect_to() {
        let access = access(&["*"], &[]);
        // No deny rules, so both families of a localhost lookup pass.
        let addrs = access.resolve_and_check("localhost", 8080).unwrap();
        assert!(!addrs.is_empty());
        assert!(addrs.iter().all(|a| a.port() == 8080));
    }
}
