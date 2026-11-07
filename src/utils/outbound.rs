//! Guards requests to addresses an admin entered, such as sign-in providers.
//!
//! The address is resolved once, checked, and the client is pinned to the checked addresses, so a changing
//! DNS answer cannot redirect the request afterwards. Redirects are never followed.
//! Link-local and metadata addresses are always refused. Private and loopback addresses and plain `http`
//! need `OIDC_ALLOW_PRIVATE_HOSTS=true`.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
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

fn is_private_v4(ip: Ipv4Addr) -> bool {
	let [a, b, c, _] = ip.octets();
	ip.is_loopback() || ip.is_private() || (a == 100 && b & 0xc0 == 64) || (a == 192 && b == 0 && c == 0) || (a == 198 && b & 0xfe == 18)
}

fn is_allowed_v4(ip: Ipv4Addr, allow_private: bool) -> bool {
	let first = ip.octets()[0];
	!(first == 0 || first >= 224 || ip.is_link_local()) && (allow_private || !is_private_v4(ip))
}

fn embedded_v4(ip: Ipv6Addr) -> Option<Ipv4Addr> {
	let segments = ip.segments();
	let from = |high: u16, low: u16| Ipv4Addr::from((u32::from(high) << 16) | u32::from(low));
	if segments[0] == 0x2002 {
		return Some(from(segments[1], segments[2]));
	}
	if segments[..6] == [0x64, 0xff9b, 0, 0, 0, 0] {
		return Some(from(segments[6], segments[7]));
	}
	ip.to_ipv4()
}

/// Whether the address may be contacted.
///
/// IPv6 addresses that carry an IPv4 address (mapped, compatible, 6to4, NAT64) are judged by that address,
/// and Teredo is always refused because its real target cannot be read from the address.
#[must_use]
pub fn is_allowed_ip(ip: IpAddr, allow_private: bool) -> bool {
	match ip {
		IpAddr::V4(v4) => is_allowed_v4(v4, allow_private),
		IpAddr::V6(v6) => {
			if v6.is_loopback() {
				return allow_private;
			}
			if let Some(v4) = embedded_v4(v6) {
				return is_allowed_v4(v4, allow_private);
			}
			let segments = v6.segments();
			let link_local = segments[0] & 0xffc0 == 0xfe80;
			let teredo = segments[0] == 0x2001 && segments[1] == 0;
			let unique_local = segments[0] & 0xfe00 == 0xfc00;
			!(v6.is_unspecified() || v6.is_multicast() || link_local || teredo) && (allow_private || !unique_local)
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
