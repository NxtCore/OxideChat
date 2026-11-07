//! Guards requests to addresses an admin entered, such as sign-in providers.
//!
//! The address is resolved once, checked, and the client is pinned to the checked addresses, so a changing
//! DNS answer cannot redirect the request afterwards. Redirects are never followed.
//! Link-local and metadata addresses are always refused. Private and loopback addresses and plain `http`
//! need `OIDC_ALLOW_PRIVATE_HOSTS=true`.

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Why an outbound request was refused.
#[derive(Debug, thiserror::Error)]
pub enum OutboundError {
	#[error("the address is not a valid https URL")]
	InvalidUrl,
	#[error("the host could not be resolved")]
	Lookup,
	#[error("the host resolves to an address that is not allowed")]
	Blocked,
	#[error("the HTTP client could not be built: {0}")]
	Client(#[from] reqwest::Error),
}

fn private_hosts_allowed() -> bool {
	std::env::var("OIDC_ALLOW_PRIVATE_HOSTS").is_ok_and(|value| value == "true")
}

fn is_private_v4(ip: std::net::Ipv4Addr) -> bool {
	let octets = ip.octets();
	ip.is_loopback() || ip.is_private() || (octets[0] == 100 && octets[1] & 0xc0 == 64)
}

/// Whether the address may be contacted.
#[must_use]
pub fn is_allowed_ip(ip: IpAddr, allow_private: bool) -> bool {
	match ip {
		IpAddr::V4(v4) => !(v4.is_unspecified() || v4.is_broadcast() || v4.is_multicast() || v4.is_link_local()) && (allow_private || !is_private_v4(v4)),
		IpAddr::V6(v6) => {
			if let Some(v4) = v6.to_ipv4_mapped() {
				return is_allowed_ip(IpAddr::V4(v4), allow_private);
			}
			let first = v6.segments()[0];
			let link_local = first & 0xffc0 == 0xfe80;
			let unique_local = first & 0xfe00 == 0xfc00;
			!(v6.is_unspecified() || v6.is_multicast() || link_local) && (allow_private || !(v6.is_loopback() || unique_local))
		}
	}
}

/// Builds a client that can only reach the checked addresses of `url`.
///
/// # Errors
///
/// Returns an error when the URL is not allowed, cannot be resolved or resolves to a blocked address.
pub async fn guarded_client(url: &str) -> Result<(reqwest::Client, reqwest::Url), OutboundError> {
	let url = reqwest::Url::parse(url).map_err(|_| OutboundError::InvalidUrl)?;
	let allow_private = private_hosts_allowed();
	if url.scheme() != "https" && !(allow_private && url.scheme() == "http") {
		return Err(OutboundError::InvalidUrl);
	}
	let host = url.host_str().ok_or(OutboundError::InvalidUrl)?.trim_matches(['[', ']']).to_string();
	let port = url.port_or_known_default().ok_or(OutboundError::InvalidUrl)?;
	let addresses: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), port)).await.map_err(|_| OutboundError::Lookup)?.collect();
	if addresses.is_empty() || addresses.iter().any(|address| !is_allowed_ip(address.ip(), allow_private)) {
		return Err(OutboundError::Blocked);
	}
	let client = reqwest::Client::builder()
		.redirect(reqwest::redirect::Policy::none())
		.timeout(REQUEST_TIMEOUT)
		.user_agent("OxideChat")
		.resolve_to_addrs(&host, &addresses)
		.build()?;
	Ok((client, url))
}
