//! [Shared] Outbound HTTP for fetching wasmhub runtimes and npm packages.
//!
//! Requests go through one configured agent rather than ureq's bare helpers,
//! whose timeouts every one default to `None`. Without them a connection that
//! never completes hangs until something further out gives up: an agent exec
//! burns its entire timeout and reports "execution timed out" for what was
//! really an unreachable registry, with nothing in stderr to say so.
//!
//! The connect budget also buys address failover. ureq divides it across the
//! addresses a host resolves to and moves to the next when one runs out, so a
//! host whose AAAA record is unreachable falls back to IPv4 instead of
//! stalling on the first address forever. That is not hypothetical: it is how
//! npm vendoring behaves on a network with broken IPv6, while wasmhub
//! downloads keep working because GitHub publishes no AAAA record.
//!
//! Reusing one agent also pools connections, which vendoring a dependency tree
//! benefits from.

use std::sync::OnceLock;
use std::time::Duration;

/// Split across a host's resolved addresses, so this bounds the whole connect
/// phase rather than a single attempt.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// Sending the request and receiving the response head. A slow *body* is
/// covered by [`BODY_TIMEOUT`] instead.
const HEAD_TIMEOUT: Duration = Duration::from_secs(30);
/// Deliberately generous: this same ceiling covers a multi-megabyte runtime
/// download on a slow link, where a tight bound would be a false alarm.
const BODY_TIMEOUT: Duration = Duration::from_secs(600);

/// The process-wide HTTP agent, built on first use.
pub fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::Agent::config_builder()
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_send_request(Some(HEAD_TIMEOUT))
            .timeout_recv_response(Some(HEAD_TIMEOUT))
            .timeout_recv_body(Some(BODY_TIMEOUT))
            .build()
            .into()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_is_configured_with_every_timeout() {
        // A `None` anywhere here is a request that can hang forever, which is
        // the failure this module exists to prevent.
        let timeouts = agent().config().timeouts();
        assert_eq!(timeouts.connect, Some(CONNECT_TIMEOUT));
        assert_eq!(timeouts.send_request, Some(HEAD_TIMEOUT));
        assert_eq!(timeouts.recv_response, Some(HEAD_TIMEOUT));
        assert_eq!(timeouts.recv_body, Some(BODY_TIMEOUT));
    }

    #[test]
    fn test_agent_is_shared() {
        assert!(std::ptr::eq(agent(), agent()), "agent should be built once");
    }
}
