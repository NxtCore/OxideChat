use super::ToolFunction;
use uuid::Uuid;

impl ToolFunction {
	pub async fn list_by_tool(pool: &sqlx::PgPool, tool_id: &Uuid) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, ToolFunction>("SELECT * FROM tool_functions WHERE tool_id = $1 ORDER BY sort_order, created_at")
			.bind(tool_id)
			.fetch_all(pool)
			.await
	}

	pub async fn list_by_tool_ids(pool: &sqlx::PgPool, tool_ids: &[Uuid]) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, ToolFunction>("SELECT * FROM tool_functions WHERE tool_id = ANY($1) ORDER BY sort_order, name")
			.bind(tool_ids)
			.fetch_all(pool)
			.await
	}

	pub async fn find_by_tool_and_name(pool: &sqlx::PgPool, tool_id: &Uuid, name: &str) -> Result<Option<Self>, sqlx::Error> {
		sqlx::query_as::<_, ToolFunction>("SELECT * FROM tool_functions WHERE tool_id = $1 AND name = $2")
			.bind(tool_id)
			.bind(name)
			.fetch_optional(pool)
			.await
	}
}
