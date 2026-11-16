#[cfg(test)]
mod tests {
	use crate::middleware::client_ip;
	use axum::{body::Body, extract::ConnectInfo, http::Request};
	use std::net::SocketAddr;

	fn request_from(peer: &str) -> axum::extract::Request {
		let mut request = Request::new(Body::empty());
		let peer: SocketAddr = peer.parse().unwrap_or_else(|_| SocketAddr::from(([0, 0, 0, 0], 0)));
		request.extensions_mut().insert(ConnectInfo(peer));
		request
	}

	#[test]
	fn client_ip_leaves_out_the_peer_port() {
		assert_eq!(client_ip(&request_from("203.0.113.7:54321")), "203.0.113.7");
		assert_eq!(client_ip(&request_from("203.0.113.7:54322")), "203.0.113.7");
		assert_eq!(client_ip(&request_from("[2001:db8::1]:443")), "2001:db8::1");
	}
}
