use super::{MAX_ROLE_NAME_LEN, RoleError};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct SaveRoleRequest {
	pub name: String,
	pub permissions: Vec<String>,
}

impl SaveRoleRequest {
	/// The trimmed name, or [`RoleError::InvalidName`] when it is empty or too long.
	///
	/// # Errors
	/// Returns [`RoleError::InvalidName`] for an empty or too long name.
	pub fn valid_name(&self) -> Result<&str, RoleError> {
		let name = self.name.trim();
		if name.is_empty() || name.chars().count() > MAX_ROLE_NAME_LEN {
			Err(RoleError::InvalidName)
		} else {
			Ok(name)
		}
	}

	/// The requested permissions, sorted and without duplicates.
	#[must_use]
	pub fn unique_permissions(&self) -> Vec<String> {
		let mut permissions = self.permissions.clone();
		permissions.sort_unstable();
		permissions.dedup();
		permissions
	}
}
