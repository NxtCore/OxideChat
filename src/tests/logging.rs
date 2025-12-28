#[cfg(test)]
mod tests {
	use crate::logging::{EntityType, LogEvent};

	#[test]
	fn test_log_event_as_str() {
		assert_eq!(LogEvent::UserLogin.as_str(), "user_login");
		assert_eq!(LogEvent::RoleAssigned.as_str(), "role_assigned");
	}

	#[test]
	fn test_log_event_from_str() {
		assert_eq!(LogEvent::from_str("user_login"), Some(LogEvent::UserLogin));
		assert_eq!(LogEvent::from_str("unknown"), None);
	}

	#[test]
	fn test_entity_type_as_str() {
		assert_eq!(EntityType::User.as_str(), "user");
		assert_eq!(EntityType::Role.as_str(), "role");
	}

	#[test]
	fn test_entity_type_from_str() {
		assert_eq!(EntityType::from_str("user"), Some(EntityType::User));
		assert_eq!(EntityType::from_str("unknown"), None);
	}
}
