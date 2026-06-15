use crate::types::BaseType;
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
pub struct ModelAccess {
	pub id: Uuid,
	pub provider_id: Option<Uuid>,
	pub model_id: Option<Uuid>,
	pub role_id: Option<Uuid>,
	pub user_id: Option<Uuid>,
	pub can_use: bool,
	pub can_configure: bool,
	pub created_at: chrono::DateTime<chrono::Utc>,
}

impl BaseType for ModelAccess {}

impl ModelAccess {
	pub async fn find_by_user(pool: &sqlx::PgPool, user_id: &Uuid) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, ModelAccess>("SELECT * FROM model_access WHERE user_id = $1")
			.bind(user_id)
			.fetch_all(pool)
			.await
	}

	pub async fn find_by_role(pool: &sqlx::PgPool, role_id: &Uuid) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, ModelAccess>("SELECT * FROM model_access WHERE role_id = $1")
			.bind(role_id)
			.fetch_all(pool)
			.await
	}

	pub async fn find_by_model(pool: &sqlx::PgPool, model_id: &Uuid) -> Result<Vec<Self>, sqlx::Error> {
		sqlx::query_as::<_, ModelAccess>("SELECT * FROM model_access WHERE model_id = $1")
			.bind(model_id)
			.fetch_all(pool)
			.await
	}

	pub async fn create(
		conn: &mut sqlx::PgConnection,
		provider_id: Option<&Uuid>,
		model_id: Option<&Uuid>,
		role_id: Option<&Uuid>,
		user_id: Option<&Uuid>,
		can_use: bool,
		can_configure: bool,
	) -> Result<Self, sqlx::Error> {
		sqlx::query_as::<_, ModelAccess>(
			r#"
			INSERT INTO model_access (provider_id, model_id, role_id, user_id, can_use, can_configure)
			VALUES ($1, $2, $3, $4, $5, $6)
			RETURNING *
			"#,
		)
		.bind(provider_id)
		.bind(model_id)
		.bind(role_id)
		.bind(user_id)
		.bind(can_use)
		.bind(can_configure)
		.fetch_one(conn)
		.await
	}
}
