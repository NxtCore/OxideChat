use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct RoleResponse {
	pub id: Uuid,
	pub name: String,
	pub is_builtin: bool,
	pub permissions: Vec<String>,
	pub user_count: i64,
}

#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct PermissionResponse {
	pub name: String,
	pub group: String,
}

impl PermissionResponse {
	/// A permission with its group: `wildcard` for `admin.*` and `settings.*`, `admin_<area>` for admin permissions,
	/// and the first part of the name for everything else.
	#[must_use]
	pub fn new(name: String) -> Self {
		let group = Self::group_of(&name);
		Self { name, group }
	}

	#[must_use]
	pub fn group_of(name: &str) -> String {
		let mut parts = name.split('.');
		let first = parts.next().unwrap_or_default();
		match parts.next() {
			None | Some("*") => "wildcard".to_owned(),
			Some(area) if first == "admin" => format!("admin_{area}"),
			Some(_) => first.to_owned(),
		}
	}
}
