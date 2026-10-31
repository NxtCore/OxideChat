use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::sync::LazyLock;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;

const MESSAGE_QUEUE: usize = 1024;
const EVICTION_INTERVAL: Duration = Duration::from_secs(60);

/// Fixed window applied to every pre-authentication rate limit.
pub const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);

static GLOBAL: LazyLock<RateLimiter> = LazyLock::new(RateLimiter::spawn);
static AUTH_IP_LIMIT: LazyLock<u64> = LazyLock::new(|| env_limit("AUTH_RATE_LIMIT_PER_MINUTE", 20));
static GATEWAY_IP_LIMIT: LazyLock<u64> = LazyLock::new(|| env_limit("GATEWAY_PRE_AUTH_IP_LIMIT_PER_MINUTE", 120));
static GATEWAY_KEY_LIMIT: LazyLock<u64> = LazyLock::new(|| env_limit("GATEWAY_PRE_AUTH_KEY_LIMIT_PER_MINUTE", 60));

/// Process-wide limiter shared by the auth middleware and the gateway pre-auth check.
#[must_use]
pub fn global() -> &'static RateLimiter {
	&GLOBAL
}

/// Per-IP limit for `/api/v1/auth/login`, `/register`, `/setup` and the OAuth routes.
#[must_use]
pub fn auth_ip_limit() -> u64 {
	*AUTH_IP_LIMIT
}

/// Per-IP limit for gateway authentication attempts.
#[must_use]
pub fn gateway_ip_limit() -> u64 {
	*GATEWAY_IP_LIMIT
}

/// Per-token-prefix limit for gateway authentication attempts.
#[must_use]
pub fn gateway_key_limit() -> u64 {
	*GATEWAY_KEY_LIMIT
}

fn env_limit(name: &str, default: u64) -> u64 {
	std::env::var(name).ok().and_then(|value| value.parse().ok()).unwrap_or(default)
}

/// Fixed-window counter limiter whose state is owned by a single Tokio task.
#[derive(Clone, Debug)]
pub struct RateLimiter {
	requests: mpsc::Sender<Message>,
}

struct Message {
	key: String,
	limit: u64,
	window: Duration,
	reply: oneshot::Sender<bool>,
}

struct Window {
	count: u64,
	reset_at: Instant,
}

#[derive(Default)]
struct Owner {
	windows: HashMap<String, Window>,
}

impl RateLimiter {
	/// Spawns the single-owner worker on the current Tokio runtime.
	#[must_use]
	pub fn spawn() -> Self {
		let (requests, receiver) = mpsc::channel(MESSAGE_QUEUE);
		tokio::spawn(Owner::default().run(receiver));
		Self { requests }
	}

	/// Returns `true` when `key` is still below `limit` in its current fixed window.
	///
	/// A `limit` of `0` disables limiting for the key.
	#[must_use]
	pub async fn allow(&self, key: &str, limit: u64, window: Duration) -> bool {
		if limit == 0 {
			return true;
		}
		let (reply, response) = oneshot::channel();
		let message = Message {
			key: key.to_string(),
			limit,
			window,
			reply,
		};
		if self.requests.send(message).await.is_err() {
			tracing::error!("rate limiter actor stopped; allowing request");
			return true;
		}
		response.await.unwrap_or(true)
	}
}

impl Owner {
	async fn run(mut self, mut receiver: mpsc::Receiver<Message>) {
		let mut eviction = tokio::time::interval(EVICTION_INTERVAL);
		loop {
			tokio::select! {
				message = receiver.recv() => match message {
					Some(message) => self.check(message),
					None => break,
				},
				_ = eviction.tick() => self.evict(),
			}
		}
	}

	fn check(&mut self, message: Message) {
		let now = Instant::now();
		let allowed = match self.windows.entry(message.key) {
			Entry::Occupied(mut entry) => {
				let window = entry.get_mut();
				if now >= window.reset_at {
					window.count = 1;
					window.reset_at = now + message.window;
					true
				} else if window.count < message.limit {
					window.count += 1;
					true
				} else {
					false
				}
			}
			Entry::Vacant(entry) => {
				entry.insert(Window {
					count: 1,
					reset_at: now + message.window,
				});
				true
			}
		};
		let _ = message.reply.send(allowed);
	}

	fn evict(&mut self) {
		let now = Instant::now();
		self.windows.retain(|_, window| now < window.reset_at);
	}
}
