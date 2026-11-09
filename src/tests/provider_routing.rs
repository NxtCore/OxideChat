#[cfg(test)]
mod tests {
	use crate::types::providers::ProviderKind;
	use crate::types::{EffectiveProviderRouting, ProviderAllowlists, ProviderRoutingError, ProviderRoutingSettings, TeamProviderRouting, normalize_allowed_providers};
	use omniference::types::ProviderRouting;

	fn slugs(list: &[&str]) -> Vec<String> {
		list.iter().map(ToString::to_string).collect()
	}

	const ROUTER: ProviderKind = ProviderKind::Openrouter;

	fn lists(allowed: Option<&[&str]>) -> ProviderAllowlists {
		allowed.map(|allowed| ProviderAllowlists::from([(ROUTER, slugs(allowed))])).unwrap_or_default()
	}

	fn installation(selector_enabled: bool, allowed: Option<&[&str]>) -> ProviderRoutingSettings {
		ProviderRoutingSettings {
			selector_enabled,
			allowed_providers: lists(allowed),
		}
	}

	fn team(selector_enabled: Option<bool>, allowed: Option<&[&str]>) -> TeamProviderRouting {
		TeamProviderRouting {
			selector_enabled,
			allowed_providers: lists(allowed),
		}
	}

	fn restricted(allowed: &[&str]) -> EffectiveProviderRouting {
		EffectiveProviderRouting {
			selector_enabled: true,
			allowed_providers: lists(Some(allowed)),
		}
	}

	mod capability {
		use super::*;

		#[test]
		fn only_routing_kinds_route_upstream() {
			assert!(ProviderKind::Openrouter.routes_upstream());
			assert!(!ProviderKind::Openai.routes_upstream());
			assert!(!ProviderKind::Anthropic.routes_upstream());
			assert!(!ProviderKind::Cortecs.routes_upstream());
		}
	}

	mod combine {
		use super::*;

		#[test]
		fn no_teams_follow_the_installation() {
			let effective = EffectiveProviderRouting::combine(&installation(true, Some(&["azure"])), &[]);
			assert!(effective.selector_enabled);
			assert_eq!(effective.allowed_providers, lists(Some(&["azure"])));
		}

		#[test]
		fn a_team_can_turn_the_selector_on_or_off() {
			assert!(EffectiveProviderRouting::combine(&installation(false, None), &[team(Some(true), None)]).selector_enabled);
			assert!(!EffectiveProviderRouting::combine(&installation(true, None), &[team(Some(false), None)]).selector_enabled);
			assert!(EffectiveProviderRouting::combine(&installation(true, None), &[team(None, None)]).selector_enabled);
		}

		#[test]
		fn off_wins_when_teams_disagree() {
			let teams = [team(Some(true), None), team(Some(false), None)];
			assert!(!EffectiveProviderRouting::combine(&installation(true, None), &teams).selector_enabled);
		}

		#[test]
		fn allowlists_intersect_across_installation_and_teams() {
			let teams = [team(None, Some(&["azure", "openai", "together"])), team(None, None), team(None, Some(&["openai", "together"]))];
			let effective = EffectiveProviderRouting::combine(&installation(false, Some(&["azure", "openai"])), &teams);
			assert_eq!(effective.allowed_providers, lists(Some(&["openai"])));
		}

		#[test]
		fn a_team_cannot_widen_the_installation_list() {
			let effective = EffectiveProviderRouting::combine(&installation(false, Some(&["azure"])), &[team(None, Some(&["openai"]))]);
			assert_eq!(effective.allowed_providers, lists(Some(&[])));
		}

		#[test]
		fn without_any_list_everything_is_allowed() {
			let effective = EffectiveProviderRouting::combine(&installation(false, None), &[team(None, None)]);
			assert!(effective.allowed_providers.is_empty());
			assert!(effective.allows(&ROUTER, "anything"));
		}

		#[test]
		fn each_gateway_keeps_its_own_list() {
			let mut settings = installation(false, Some(&["azure"]));
			settings.allowed_providers.insert(ProviderKind::Cortecs, slugs(&["nebius"]));
			let effective = EffectiveProviderRouting::combine(&settings, &[team(None, Some(&["azure", "openai"]))]);
			assert_eq!(effective.allowed_providers.get(&ROUTER), Some(&slugs(&["azure"])));
			assert_eq!(effective.allowed_providers.get(&ProviderKind::Cortecs), Some(&slugs(&["nebius"])));
			assert!(!effective.allows(&ProviderKind::Cortecs, "azure"));
			assert!(effective.allows(&ProviderKind::Openai, "anything"));
		}
	}

	mod restrict {
		use super::*;

		#[test]
		fn unrestricted_routing_is_left_alone() {
			let effective = EffectiveProviderRouting::default();
			assert!(effective.restrict(&ROUTER, None).unwrap().is_none());
		}

		#[test]
		fn automatic_routing_gets_the_allowlist() {
			let routing = restricted(&["azure", "openai"]).restrict(&ROUTER, None).unwrap().unwrap();
			assert_eq!(routing.only, Some(slugs(&["azure", "openai"])));
			assert_eq!(routing.order, None);
		}

		#[test]
		fn a_preferred_allowed_provider_keeps_its_order_within_the_allowlist() {
			let preferred = ProviderRouting {
				order: Some(slugs(&["OpenAI"])),
				..Default::default()
			};
			let routing = restricted(&["azure", "openai"]).restrict(&ROUTER, Some(preferred)).unwrap().unwrap();
			assert_eq!(routing.order, Some(slugs(&["OpenAI"])));
			assert_eq!(routing.only, Some(slugs(&["azure", "openai"])));
		}

		#[test]
		fn a_locked_allowed_provider_stays_locked() {
			let locked = ProviderRouting {
				only: Some(slugs(&["openai"])),
				allow_fallbacks: Some(false),
				..Default::default()
			};
			let routing = restricted(&["azure", "openai"]).restrict(&ROUTER, Some(locked)).unwrap().unwrap();
			assert_eq!(routing.only, Some(slugs(&["openai"])));
			assert_eq!(routing.allow_fallbacks, Some(false));
		}

		#[test]
		fn a_provider_outside_the_allowlist_is_refused() {
			let preferred = ProviderRouting {
				order: Some(slugs(&["together"])),
				..Default::default()
			};
			assert_eq!(restricted(&["azure"]).restrict(&ROUTER, Some(preferred)).err(), Some(ProviderRoutingError::NotAllowed("together".to_string())));
		}

		#[test]
		fn variants_of_an_allowed_provider_are_allowed() {
			let effective = restricted(&["deepinfra"]);
			assert!(effective.allows(&ROUTER, "DeepInfra/fp8"));
			assert!(!effective.allows(&ROUTER, "deepinfra-turbo"));
			assert!(!effective.allows(&ROUTER, "deep"));
		}

		#[test]
		fn a_list_for_another_gateway_does_not_restrict_this_one() {
			let effective = EffectiveProviderRouting {
				selector_enabled: true,
				allowed_providers: ProviderAllowlists::from([(ProviderKind::Cortecs, slugs(&["nebius"]))]),
			};
			assert!(effective.restrict(&ROUTER, None).unwrap().is_none());
		}

		#[test]
		fn an_empty_allowlist_refuses_everything() {
			assert_eq!(restricted(&[]).restrict(&ROUTER, None).err(), Some(ProviderRoutingError::NoneAllowed));
		}
	}

	mod normalize {
		use super::*;

		#[test]
		fn slugs_are_trimmed_lowercased_and_deduplicated() {
			assert_eq!(normalize_allowed_providers(&slugs(&[" Azure ", "openai", "azure"])), Some(slugs(&["azure", "openai"])));
		}

		#[test]
		fn empty_or_malformed_lists_are_rejected() {
			assert_eq!(normalize_allowed_providers(&[]), None);
			assert_eq!(normalize_allowed_providers(&slugs(&[""])), None);
			assert_eq!(normalize_allowed_providers(&slugs(&["no spaces"])), None);
			assert_eq!(normalize_allowed_providers(&["x".repeat(65)]), None);
		}

		#[test]
		fn settings_without_a_list_stay_unrestricted() {
			let settings = team(Some(true), None);
			assert_eq!(settings.normalized(), Some(settings));
		}

		#[test]
		fn lists_are_normalized_per_gateway() {
			let normalized = team(None, Some(&[" Azure ", "azure"])).normalized();
			assert_eq!(normalized.map(|settings| settings.allowed_providers), Some(lists(Some(&["azure"]))));
			assert_eq!(team(None, Some(&[])).normalized(), None);
		}

		#[test]
		fn a_list_for_a_direct_provider_kind_is_rejected() {
			let settings = TeamProviderRouting {
				selector_enabled: None,
				allowed_providers: ProviderAllowlists::from([(ProviderKind::Openai, slugs(&["azure"]))]),
			};
			assert_eq!(settings.normalized(), None);
		}
	}

	mod database {
		use super::*;
		use crate::config::Config;
		use crate::tests::support::insert_user;
		use crate::types::teams::{CreateTeamRequest, SaveTeamRequest, Team};
		use sqlx::PgPool;
		use uuid::Uuid;

		async fn create_team(pool: &PgPool, name: &str, routing: (Option<bool>, Option<&[&str]>)) -> Result<Team, sqlx::Error> {
			let team = Team::create(
				pool,
				&CreateTeamRequest {
					name: name.to_string(),
					description: None,
					allow_all_models: None,
					member_ids: None,
					provider_ids: None,
					model_ids: None,
				},
			)
			.await?;
			sqlx::query("UPDATE teams SET provider_selector_enabled = $2, allowed_upstream_providers = $3 WHERE id = $1")
				.bind(team.id)
				.bind(routing.0)
				.bind(sqlx::types::Json(lists(routing.1)))
				.execute(pool)
				.await?;
			Ok(team)
		}

		async fn join(pool: &PgPool, team_id: Uuid, user_id: Uuid) -> Result<(), sqlx::Error> {
			sqlx::query("INSERT INTO team_members (team_id, user_id) VALUES ($1, $2)").bind(team_id).bind(user_id).execute(pool).await?;
			Ok(())
		}

		#[sqlx::test(migrations = "./migrations")]
		async fn a_user_gets_the_most_restrictive_of_their_teams(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
			Config::init(&pool).await;
			let user = insert_user(&pool, "routing_member").await?;
			let first = create_team(&pool, "Routing A", (Some(true), Some(&["azure", "openai"]))).await?;
			let second = create_team(&pool, "Routing B", (None, Some(&["openai", "together"]))).await?;
			join(&pool, first.id, user).await?;
			join(&pool, second.id, user).await?;

			let effective = EffectiveProviderRouting::for_user(&pool, &user).await?;
			assert!(effective.selector_enabled);
			assert_eq!(effective.allowed_providers, lists(Some(&["openai"])));

			let team_only = EffectiveProviderRouting::for_team(&pool, &first.id).await?;
			assert_eq!(team_only.allowed_providers, lists(Some(&["azure", "openai"])));
			Ok(())
		}

		#[sqlx::test(migrations = "./migrations")]
		async fn team_save_stores_the_routing_settings(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
			let actor = insert_user(&pool, "routing_actor").await?;
			let team = create_team(&pool, "Routing save", (None, None)).await?;
			let request = SaveTeamRequest {
				name: "Routing save".to_string(),
				description: None,
				allow_all_models: true,
				default_model_id: None,
				members: Vec::new(),
				provider_ids: Vec::new(),
				model_ids: Vec::new(),
				provider_routing: team_routing(),
			};

			let saved = team.save(&pool, &actor, &request).await?;
			assert_eq!(saved.provider_routing(), team_routing());
			assert_eq!(saved.detailed_response(&pool).await?.provider_routing, team_routing());
			Ok(())
		}

		#[sqlx::test(migrations = "./migrations")]
		async fn an_unsynced_gateway_lists_its_endpoint_options(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
			Config::init(&pool).await;
			let provider: Uuid =
				sqlx::query_scalar("INSERT INTO providers (kind, name, base_url, is_enabled) VALUES ('OPENROUTER', 'Unreachable router', 'http://127.0.0.1:9', true) RETURNING id")
					.fetch_one(&pool)
					.await?;
			let catalog: Uuid = sqlx::query_scalar("INSERT INTO gateway_catalog_models (provider_id, source_gateway, gateway_model_id) VALUES ($1, 'openrouter', 'a/b') RETURNING id")
				.bind(provider)
				.fetch_one(&pool)
				.await?;
			sqlx::query("INSERT INTO gateway_model_provider_options (catalog_model_id, provider_slug, provider_name) VALUES ($1, 'DeepInfra/fp8', 'DeepInfra'), ($1, 'deepinfra', 'DeepInfra'), ($1, 'azure', 'Azure')")
				.bind(catalog)
				.execute(&pool)
				.await?;

			let gateways = crate::types::UpstreamGatewayResponse::load_all(&pool).await?;
			assert_eq!(gateways.len(), 1);
			assert_eq!(gateways[0].kind, ROUTER);
			assert!(gateways[0].synced_at.is_none());
			let listed: Vec<&str> = gateways[0].providers.iter().map(|provider| provider.slug.as_str()).collect();
			assert_eq!(listed, ["azure", "deepinfra", "deepinfra/fp8"]);
			Ok(())
		}

		#[sqlx::test(migrations = "./migrations")]
		async fn without_routing_providers_no_gateway_is_listed(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
			Config::init(&pool).await;
			assert!(crate::types::UpstreamGatewayResponse::load_all(&pool).await?.is_empty());
			Ok(())
		}

		struct CapturingHandler {
			sender: std::sync::mpsc::Sender<Option<ProviderRouting>>,
		}

		#[async_trait::async_trait]
		impl omniference::middleware::RequestHandler for CapturingHandler {
			async fn handle(&self, request: omniference::types::ChatRequestIR, _cancel: tokio_util::sync::CancellationToken) -> anyhow::Result<omniference::middleware::ChatStream> {
				self.sender.send(request.provider_routing)?;
				Ok(Box::new(futures_util::stream::empty()))
			}
		}

		fn job_state(db: PgPool) -> std::sync::Arc<crate::types::JobState> {
			std::sync::Arc::new(crate::types::JobState {
				db,
				gateway_limiter: crate::types::GatewayLimiter::spawn(),
				budget_ledger: crate::types::BudgetLedger::spawn(),
				login_limiter: crate::utils::login_limit::LoginRateLimiter::spawn(),
				mcp_pool: crate::utils::tools::McpConnectionPool::new(),
				client_tool_pending: crate::types::state::ClientToolPending::new(),
			})
		}

		async fn routed_model(pool: &PgPool, kind: &str) -> Result<Uuid, sqlx::Error> {
			let provider: Uuid = sqlx::query_scalar("INSERT INTO providers (kind, name, base_url, is_enabled) VALUES ($1::provider_kind, $2, 'https://example.com', true) RETURNING id")
				.bind(kind)
				.bind(format!("Routing {}", Uuid::new_v4()))
				.fetch_one(pool)
				.await?;
			sqlx::query_scalar("INSERT INTO models (provider_id, model_id, display_name, is_enabled) VALUES ($1, 'routing-model', 'routing-model', true) RETURNING id")
				.bind(provider)
				.fetch_one(pool)
				.await
		}

		async fn run_middleware(pool: &PgPool, metadata: &[(&str, Uuid)], routing: Option<ProviderRouting>) -> anyhow::Result<Option<ProviderRouting>> {
			use omniference::middleware::Middleware;
			let middleware = crate::types::ProviderRoutingMiddleware::new(job_state(pool.clone()));
			let (sender, receiver) = std::sync::mpsc::channel();
			let request = omniference::types::ChatRequestIR {
				provider_routing: routing,
				metadata: metadata.iter().map(|(key, value)| ((*key).to_string(), value.to_string())).collect(),
				..Default::default()
			};
			let _stream = middleware.handle(request, tokio_util::sync::CancellationToken::new(), &CapturingHandler { sender }).await?;
			Ok(receiver.recv()?)
		}

		#[sqlx::test(migrations = "./migrations")]
		async fn the_middleware_restricts_routed_requests_to_the_subjects_allowlist(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
			Config::init(&pool).await;
			crate::i18n::I18n::init(&pool).await;
			let user = insert_user(&pool, "routing_middleware").await?;
			let team = create_team(&pool, "Routing middleware", (None, Some(&["azure", "openai"]))).await?;
			join(&pool, team.id, user).await?;
			let model = routed_model(&pool, "OPENROUTER").await?;

			let web_chat = run_middleware(&pool, &[("oxide_model_id", model), ("oxide_user_id", user)], None).await?;
			assert_eq!(web_chat.and_then(|routing| routing.only), Some(slugs(&["azure", "openai"])));

			let gateway = run_middleware(&pool, &[("oxide_model_id", model), ("oxide_team_id", team.id)], None).await?;
			assert_eq!(gateway.and_then(|routing| routing.only), Some(slugs(&["azure", "openai"])));

			let refused = ProviderRouting {
				order: Some(slugs(&["together"])),
				..Default::default()
			};
			let error = run_middleware(&pool, &[("oxide_model_id", model), ("oxide_user_id", user)], Some(refused)).await.err().ok_or("expected a refusal")?;
			let adapter = error.downcast::<omniference::adapter::AdapterError>()?;
			assert!(matches!(adapter, omniference::adapter::AdapterError::Provider { http_status: Some(403), .. }));
			Ok(())
		}

		#[sqlx::test(migrations = "./migrations")]
		async fn the_middleware_leaves_direct_providers_alone(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
			Config::init(&pool).await;
			let user = insert_user(&pool, "routing_direct").await?;
			let team = create_team(&pool, "Routing direct", (None, Some(&["azure"]))).await?;
			join(&pool, team.id, user).await?;
			let model = routed_model(&pool, "OPENAI").await?;

			let routing = run_middleware(&pool, &[("oxide_model_id", model), ("oxide_user_id", user)], None).await?;
			assert!(routing.is_none());
			Ok(())
		}

		async fn fake_openrouter() -> Result<String, std::io::Error> {
			use serde_json::json;
			let router = axum::Router::new().route(
				"/v1/providers",
				axum::routing::get(|| async {
					axum::Json(json!({"data": [
						{"name": "Azure", "slug": "azure", "headquarters": "US", "datacenters": ["US", "SE"]},
						{"name": "Mistral", "slug": "Mistral", "headquarters": "FR", "datacenters": null}
					]}))
				}),
			);
			let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
			let address = listener.local_addr()?;
			tokio::spawn(async move { axum::serve(listener, router).await });
			Ok(format!("http://{address}"))
		}

		async fn openrouter_provider(pool: &PgPool, base_url: &str) -> Result<Uuid, Box<dyn std::error::Error>> {
			let api_key = crate::utils::encryption::encrypt_api_key("sk-or-test")?;
			let provider = sqlx::query_scalar("INSERT INTO providers (kind, name, base_url, api_key, is_enabled) VALUES ('OPENROUTER', 'Router', $1, $2, true) RETURNING id")
				.bind(base_url)
				.bind(api_key)
				.fetch_one(pool)
				.await?;
			Ok(provider)
		}

		#[sqlx::test(migrations = "./migrations")]
		async fn a_sync_stores_the_providers(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
			Config::init(&pool).await;
			crate::utils::encryption::init_for_test([0x42; 32])?;
			let base_url = fake_openrouter().await?;
			openrouter_provider(&pool, &base_url).await?;

			let synced = crate::types::UpstreamGatewayResponse::sync_all(&pool).await?;
			assert!(synced.failed.is_empty());
			let gateway = &synced.gateways[0];
			assert!(gateway.synced_at.is_some());
			assert!(gateway.provider_id.is_some());
			let listed: Vec<(&str, Vec<String>)> = gateway.providers.iter().map(|provider| (provider.slug.as_str(), provider.datacenters.clone())).collect();
			assert_eq!(listed, [("azure", slugs(&["US", "SE"])), ("mistral", Vec::new())]);

			let reloaded = crate::types::UpstreamGatewayResponse::load_all(&pool).await?;
			assert_eq!(reloaded[0].providers, gateway.providers);
			Ok(())
		}

		#[sqlx::test(migrations = "./migrations")]
		async fn a_failed_sync_keeps_the_previous_snapshot(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {
			Config::init(&pool).await;
			crate::utils::encryption::init_for_test([0x42; 32])?;
			let base_url = fake_openrouter().await?;
			let provider = openrouter_provider(&pool, &base_url).await?;
			crate::types::UpstreamGatewayResponse::sync_all(&pool).await?;
			sqlx::query("UPDATE providers SET base_url = 'http://127.0.0.1:9' WHERE id = $1").bind(provider).execute(&pool).await?;

			let synced = crate::types::UpstreamGatewayResponse::sync_all(&pool).await?;
			assert_eq!(synced.failed, [ROUTER]);
			assert_eq!(synced.gateways[0].providers.len(), 2);
			Ok(())
		}

		fn team_routing() -> TeamProviderRouting {
			team(Some(false), Some(&["azure"]))
		}
	}
}
