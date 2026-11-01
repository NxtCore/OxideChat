#[cfg(test)]
mod tests {
	use crate::types::permissions::ALL;
	use crate::types::models::{ModelListAdmin, ModelListPublic, ProviderTab};
	use crate::types::catalog::ModelProviderOptions;
	use crate::types::providers::{ProviderBillingOverviewResponse, ProviderResponse, TestProviderResponse, UpdateProviderBillingRequest};
	use crate::types::teams::TeamRole;
	use crate::types::user::{SessionResponse, UserListResponse};
	use crate::types::*;
	use std::error::Error;
	use std::fmt::Write;
	use std::fs;
	use ts_rs::{Config, Dummy, TS};

	const OUT_DIR: &str = "frontend/types/generated";

	#[test]
	fn generate_frontend_types() -> Result<(), Box<dyn Error>> {
		if fs::metadata(OUT_DIR).is_ok() {
			fs::remove_dir_all(OUT_DIR)?;
		}
		let cfg = Config::new().with_large_int("number").with_out_dir(OUT_DIR);

		macro_rules! export {
			($($ty:ty),* $(,)?) => { $(<$ty as TS>::export_all(&cfg)?;)* };
		}
		export!(
			AdminConfigResponse,
			SessionResponse,
			TestProviderResponse,
			TranslationsResponse,
			AnalyticsDayModelRow,
			BaseResponse,
			GlobalConfigResponse,
			ModelProviderOptions,
			ProviderResponse,
			AnalyticsRow,
			ApiKeyAnalyticsRow,
			BudgetAssignmentInfo,
			BudgetResetEventResponse,
			BudgetResetRequest,
			BudgetResponse,
			BudgetTeamSummaryResponse,
			ChatResponse,
			CreateBudgetRequest,
			CreateChatRequest,
			CreateGatewayKeyRequest,
			CreateMcpServerRequest,
			CreateWorkspaceRequest,
			DeleteWorkspaceParams,
			EffectiveBudgetResponse,
			GatewayAdminKeyDetailResponse,
			GatewayAdminTeamResponse,
			GatewayAuditEntryResponse,
			GatewayEventResponse,
			GatewayKeyBudgetResponse,
			GatewayKeyRotationResponse,
			GatewayKeyRuleResponse,
			GatewayManagedTeamResponse,
			GatewayTeamKeysResponse,
			GatewayTeamResponse,
			McpDiscoveryResponse,
			McpServerResponse,
			ModelListAdmin,
			ModelListPublic,
			Page<Dummy>,
			PreferencesResponse,
			ProviderBillingOverviewResponse,
			ProviderTab,
			RequestLogResponse,
			RequestLogSummary,
			SendMessageRequest,
			SetGatewayKeyBudgetRequest,
			TeamBudgetOverviewResponse,
			TeamDetailedResponse,
			TeamResponse,
			ToolResponse,
			UpdateChatRequest,
			UpdateMcpServerRequest,
			UpdatePreferencesRequest,
			UpdateProviderBillingRequest,
			UpdateWorkspaceRequest,
			UserBudgetOverviewResponse,
			UserBudgetStatus,
			TeamRole,
			UserListResponse,
			UserResponse,
			WorkspaceDeleteAction,
			WorkspaceResponse,
		);

		let mut permissions = String::new();
		for (name, value) in ALL {
			writeln!(permissions, "export const {name} = '{value}';")?;
		}
		fs::write(format!("{OUT_DIR}/permissions.ts"), permissions)?;
		Ok(())
	}
}
