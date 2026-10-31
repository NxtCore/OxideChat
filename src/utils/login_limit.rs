//! Brute-force protection for the login endpoint.
//!
//! A single Tokio actor owns the failure windows; handles only send messages
//! over channels, so no locking is needed.

use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;

const MAX_FAILURES: usize = 10;
const WINDOW: Duration = Duration::from_secs(5 * 60);
const REQUEST_QUEUE: usize = 256;
const SWEEP_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Clone, Debug)]
/// Channel handle for the process-local login attempt owner.
pub struct LoginRateLimiter {
	requests: mpsc::Sender<Request>,
}

enum Request {
	Allow { ip: IpAddr, email: String, reply: oneshot::Sender<bool> },
	Failure { ip: IpAddr, email: String },
}

#[derive(Default)]
struct Owner {
	windows: HashMap<(IpAddr, Option<String>), VecDeque<Instant>>,
}

impl LoginRateLimiter {
	/// Starts the single-owner worker on the current Tokio runtime.
	#[must_use]
	pub fn spawn() -> Self {
		let (requests, receiver) = mpsc::channel(REQUEST_QUEUE);
		tokio::spawn(Owner::default().run(receiver));
		Self { requests }
	}

	/// Returns `true` while another login attempt is allowed for this IP and email.
	///
	/// Fails open when the limiter actor is unavailable.
	#[must_use]
	pub async fn allow(&self, ip: IpAddr, email: &str) -> bool {
		let (reply, response) = oneshot::channel();
		let request = Request::Allow {
			ip,
			email: email.to_string(),
			reply,
		};
		if self.requests.send(request).await.is_err() {
			return true;
		}
		response.await.unwrap_or(true)
	}

	/// Records a failed login attempt for this IP and email.
	pub fn record_failure(&self, ip: IpAddr, email: &str) {
		let request = Request::Failure {
			ip,
			email: email.to_string(),
		};
		if self.requests.try_send(request).is_err() {
			tracing::warn!("login limiter queue full or closed; failed attempt not recorded");
		}
	}
}

impl Owner {
	async fn run(mut self, mut receiver: mpsc::Receiver<Request>) {
		let mut sweep = tokio::time::interval(SWEEP_INTERVAL);
		loop {
			tokio::select! {
				message = receiver.recv() => match message {
					Some(message) => self.serve(message),
					None => break,
				},
				_ = sweep.tick() => self.sweep(),
			}
		}
	}

	fn serve(&mut self, message: Request) {
		match message {
			Request::Allow { ip, email, reply } => {
				if reply.is_closed() {
					return;
				}
				let allowed = self.allowed(ip, &email, Instant::now());
				let _ = reply.send(allowed);
			}
			Request::Failure { ip, email } => self.record(ip, &email),
		}
	}

	fn allowed(&mut self, ip: IpAddr, email: &str, now: Instant) -> bool {
		let mut limited = false;
		for key in [(ip, None), (ip, Some(email.to_string()))] {
			let window = self.windows.entry(key).or_default();
			prune(window, now);
			if window.len() >= MAX_FAILURES {
				limited = true;
			}
		}
		!limited
	}

	fn record(&mut self, ip: IpAddr, email: &str) {
		let now = Instant::now();
		for key in [(ip, None), (ip, Some(email.to_string()))] {
			let window = self.windows.entry(key).or_default();
			prune(window, now);
			window.push_back(now);
		}
	}

	fn sweep(&mut self) {
		let now = Instant::now();
		self.windows.retain(|_, window| {
			prune(window, now);
			!window.is_empty()
		});
	}
}

fn prune(window: &mut VecDeque<Instant>, now: Instant) {
	while window.front().is_some_and(|front| now.saturating_duration_since(*front) >= WINDOW) {
		window.pop_front();
	}
}
