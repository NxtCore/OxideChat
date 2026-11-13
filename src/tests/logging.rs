#[cfg(test)]
mod tests {
	use crate::logging::{EntityType, LogEvent};

	#[test]
	fn test_log_event_as_str() {
		assert_eq!(LogEvent::UserLogin.as_str(), "user_login");
	}

	#[test]
	fn test_entity_type_as_str() {
		assert_eq!(EntityType::User.as_str(), "user");
		assert_eq!(EntityType::Session.as_str(), "session");
	}
}
