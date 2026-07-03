use crate::types::Team;
use sqlx::PgPool;
use uuid::Uuid;

pub struct PolicyResolver;

impl PolicyResolver {
	pub async fn can_use_model(pool: &PgPool, user_id: &Uuid, model_id: &Uuid) -> Result<bool, sqlx::Error> {
		Team::user_can_use_model(pool, user_id, model_id).await
	}
}
