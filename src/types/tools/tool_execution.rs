use super::ToolExecution;
use uuid::Uuid;

impl ToolExecution {
	pub async fn create_for_message_batch(pool: &sqlx::PgPool, message_id: &Uuid, executions: &[crate::types::ToolExecutionInternal]) -> Result<(), sqlx::Error> {
		if executions.is_empty() {
			return Ok(());
		}

		let mut tx = pool.begin().await?;
		for exec in executions {
			sqlx::query(
				r#"
				INSERT INTO tool_executions (message_id, tool_call_id, input_args, output, error, execution_ms, tool_id, tool_function)
				VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
				"#,
			)
			.bind(message_id)
			.bind(&exec.call_id)
			.bind(&exec.args)
			.bind(&exec.output)
			.bind(&exec.error)
			.bind(exec.execution_ms)
			.bind(exec.tool_id)
			.bind(exec.function_id)
			.execute(&mut *tx)
			.await?;
		}
		tx.commit().await
	}
}
