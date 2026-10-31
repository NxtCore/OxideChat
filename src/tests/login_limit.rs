#[cfg(test)]
mod tests {
	use crate::utils::login_limit::LoginRateLimiter;
	use std::net::{IpAddr, Ipv4Addr};

	fn ip() -> IpAddr {
		IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7))
	}

	#[tokio::test]
	async fn allows_until_ten_failures_then_blocks() {
		let limiter = LoginRateLimiter::spawn();
		for _ in 0..10 {
			assert!(limiter.allow(ip(), "a@example.com").await);
			limiter.record_failure(ip(), "a@example.com");
		}
		assert!(!limiter.allow(ip(), "a@example.com").await);
	}

	#[tokio::test]
	async fn ip_window_blocks_other_emails_from_same_ip() {
		let limiter = LoginRateLimiter::spawn();
		for _ in 0..10 {
			limiter.record_failure(ip(), "a@example.com");
		}
		assert!(!limiter.allow(ip(), "b@example.com").await);
	}

	#[tokio::test]
	async fn different_ip_is_independent() {
		let limiter = LoginRateLimiter::spawn();
		let other = IpAddr::V4(Ipv4Addr::new(198, 51, 100, 9));
		for _ in 0..10 {
			limiter.record_failure(ip(), "a@example.com");
		}
		assert!(limiter.allow(other, "a@example.com").await);
	}

	#[tokio::test]
	async fn successful_login_is_not_a_failure() {
		let limiter = LoginRateLimiter::spawn();
		for _ in 0..20 {
			assert!(limiter.allow(ip(), "a@example.com").await);
		}
	}
}
