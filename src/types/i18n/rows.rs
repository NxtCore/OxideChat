use sqlx::FromRow;
use uuid::Uuid;

#[derive(FromRow)]
pub(super) struct IdRow {
	pub id: Uuid,
}
