use sqlx::FromRow;

#[derive(Debug, FromRow)]
pub(super) struct CountRow {
	pub count: i64,
}
