//! The shared shape for assigning something (a budget, a model, a preset) to users, teams, roles or keys.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Request Types

/// One target in a replace request; `everyone` has no ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct AssignmentRef {
	pub kind: AssignmentKind,
	#[serde(default)]
	#[cfg_attr(test, ts(optional))]
	pub id: Option<Uuid>,
	/// What the target may do, for features whose targets hold a role; a missing role is the weakest one.
	#[serde(default)]
	#[cfg_attr(test, ts(optional))]
	pub role: Option<AssignmentRole>,
}

/// Body of every `PUT …/assignments`: the full list of targets that should remain.
#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ReplaceAssignmentsRequest {
	pub targets: Vec<AssignmentRef>,
}

impl ReplaceAssignmentsRequest {
	/// Whether every target is of an allowed kind and carries an ID exactly when its kind needs one.
	#[must_use]
	pub fn is_valid_for(&self, kinds: &[AssignmentKind]) -> bool {
		self.targets.iter().all(|target| kinds.contains(&target.kind) && target.id.is_some() != (target.kind == AssignmentKind::Everyone))
	}

	/// The distinct IDs of all targets of `kind`.
	#[must_use]
	pub fn ids(&self, kind: AssignmentKind) -> Vec<Uuid> {
		let mut ids: Vec<Uuid> = self.targets.iter().filter(|target| target.kind == kind).filter_map(|target| target.id).collect();
		ids.sort_unstable();
		ids.dedup();
		ids
	}
}

// Response Types

/// What an assigned target may do with the thing, for features that tell viewers and editors apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum AssignmentRole {
	Viewer,
	Editor,
}

impl AssignmentRole {
	/// The value stored in the database.
	#[must_use]
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Viewer => "viewer",
			Self::Editor => "editor",
		}
	}

	/// Parses a stored role; unknown values are treated as a viewer.
	#[must_use]
	pub fn from_stored(value: &str) -> Self {
		if value == "editor" { Self::Editor } else { Self::Viewer }
	}
}

/// What something can be assigned to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum AssignmentKind {
	User,
	Team,
	Role,
	Key,
	Everyone,
}

impl AssignmentKind {
	/// Parses the kind a query returned; unknown values are treated as a user.
	#[must_use]
	pub fn from_stored(value: &str) -> Self {
		match value {
			"team" => Self::Team,
			"role" => Self::Role,
			"key" => Self::Key,
			"everyone" => Self::Everyone,
			_ => Self::User,
		}
	}
}

/// One assigned target; `inherited` targets come from a wider rule and the `owner` target owns the thing, so neither can be removed here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct AssignmentTarget {
	pub kind: AssignmentKind,
	pub id: Option<Uuid>,
	pub label: String,
	pub inherited: bool,
	pub owner: bool,
	/// The target's role, for features whose targets hold one.
	pub role: Option<AssignmentRole>,
}

impl From<(String, Uuid, String, bool)> for AssignmentTarget {
	fn from((kind, id, label, inherited): (String, Uuid, String, bool)) -> Self {
		(AssignmentKind::from_stored(&kind), id, label, inherited).into()
	}
}

impl From<(AssignmentKind, Uuid, String, bool)> for AssignmentTarget {
	fn from((kind, id, label, inherited): (AssignmentKind, Uuid, String, bool)) -> Self {
		Self {
			kind,
			id: Some(id),
			label,
			inherited,
			owner: false,
			role: None,
		}
	}
}

/// Response of every `GET` and `PUT …/assignments`.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct AssignmentsResponse {
	/// The kinds this feature accepts for the caller.
	pub kinds: Vec<AssignmentKind>,
	/// Whether the feature holds exactly one target.
	pub single: bool,
	/// The roles a target can hold, weakest first; empty for features without roles.
	pub roles: Vec<AssignmentRole>,
	pub targets: Vec<AssignmentTarget>,
}
