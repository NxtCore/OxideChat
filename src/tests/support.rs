#![cfg(test)]

use crate::types::PageQuery;
use sqlx::PgPool;
use uuid::Uuid;

pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

pub async fn insert_user(pool: &PgPool, name: &str) -> Result<Uuid, sqlx::Error> {
	sqlx::query_scalar("INSERT INTO users (email, username, password_hash) VALUES ($1, $2, 'hash') RETURNING id")
		.bind(format!("{name}@example.com"))
		.bind(name)
		.fetch_one(pool)
		.await
}

pub async fn create_user(pool: &PgPool) -> TestResult<Uuid> {
	Ok(insert_user(pool, &format!("user{}", Uuid::new_v4().simple())).await?)
}

pub fn paged(page: i64, size: i64) -> PageQuery {
	PageQuery { page: Some(page), size: Some(size) }
}
