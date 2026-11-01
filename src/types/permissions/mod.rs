macro_rules! permissions {
	($($name:ident = $value:literal),* $(,)?) => {
		pub mod consts {
			$(#[allow(dead_code)] pub const $name: &str = $value;)*
		}

		#[cfg(test)]
		pub const ALL: &[(&str, &str)] = &[$((stringify!($name), $value)),*];
	};
}

permissions! {
	ADMIN_USERS_VIEW = "admin.users.view",
	ADMIN_USERS_EDIT = "admin.users.edit",
	ADMIN_TEAMS_VIEW = "admin.teams.view",
	ADMIN_TEAMS_EDIT = "admin.teams.edit",
	ADMIN_BUDGETS_VIEW = "admin.budgets.view",
	ADMIN_BUDGETS_EDIT = "admin.budgets.edit",
	ADMIN_ANALYTICS_VIEW = "admin.analytics.view",
	ADMIN_TOOLS_VIEW = "admin.tools.view",
	ADMIN_TOOLS_EDIT = "admin.tools.edit",
	ADMIN_PROVIDERS_VIEW = "admin.providers.view",
	ADMIN_PROVIDERS_EDIT = "admin.providers.edit",
	ADMIN_CONFIG_VIEW = "admin.config.view",
	ADMIN_CONFIG_EDIT = "admin.config.edit",
	ADMIN_DEVELOPER_VIEW = "admin.developer.view",
	ADMIN_DEVELOPER_KEYS_EDIT = "admin.developer.keys.edit",
	ADMIN_DEVELOPER_LIMITS_EDIT = "admin.developer.limits.edit",
	ADMIN_DEVELOPER_ACCESS_EDIT = "admin.developer.access.edit",
	ADMIN_DEVELOPER_AUDIT_VIEW = "admin.developer.audit.view",
}
