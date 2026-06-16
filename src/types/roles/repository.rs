use super::Role;
use super::rows::CountRow;

impl Role {
	pub async fn exists(pool: &sqlx::PgPool, name: &str) -> Result<bool, sqlx::Error> {
		let count: CountRow = sqlx::query_as("SELECT COUNT(*) as count FROM roles WHERE name = $1")
			.bind(name)
			.fetch_one(pool)
			.await?;
		Ok(count.count > 0)
	}

	pub async fn list_all(pool: &sqlx::PgPool) -> Result<Vec<Role>, sqlx::Error> {
		sqlx::query_as::<_, Role>("SELECT * FROM roles ORDER BY name ASC").fetch_all(pool).await
	}
}
