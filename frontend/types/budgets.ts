export interface Budget {
	id: string;
	name: string;
	description: string | null;
	amount: string;
	kind: 'pooled' | 'per_user';
	interval: 'daily' | 'weekly' | 'monthly' | 'total';
	reset_strategy: 'calendar' | 'rolling' | 'anchored';
	on_exceed: 'block' | 'warn' | 'allow';
	is_enabled: boolean;
	created_at: string;
	updated_at: string;
}

export interface EffectiveBudget {
	budget: Budget;
	assignment_id: string;
	team_id: string | null;
	user_id: string | null;
	amount: string;
	spent: string;
	remaining: string;
	window_start: string;
	resets_at: string | null;
	on_exceed: 'block' | 'warn' | 'allow';
	exhausted: boolean;
}

export interface UserBudgetStatus {
	budgets: EffectiveBudget[];
	decision: 'block' | 'warn' | 'allow';
	blocked_model_ids: string[];
}

export interface AnalyticsRow {
	id: string | null;
	label: string;
	input_tokens: number;
	output_tokens: number;
	reasoning_tokens: number;
	cost_total: string;
	request_count: number;
}

export interface AnalyticsDayModelRow {
	day: string;
	model_id: string | null;
	model_name: string;
	input_tokens: number;
	output_tokens: number;
	reasoning_tokens: number;
	cost_total: string;
	request_count: number;
}

export interface BudgetAssignmentInfo {
	id: string;
	budget_id: string;
	team_id: string | null;
	team_name: string | null;
	user_id: string | null;
	user_label: string | null;
	assigned_at: string;
}

export interface BudgetPayload {
	name: string;
	description?: string | null;
	amount: string;
	kind: 'pooled' | 'per_user';
	interval: 'daily' | 'weekly' | 'monthly' | 'total';
	reset_strategy: 'calendar' | 'rolling' | 'anchored';
	on_exceed: 'block' | 'warn' | 'allow';
	is_enabled?: boolean;
}

export interface BudgetTeamSummary {
	id: string;
	name: string;
	is_default: boolean;
}

export interface UserBudgetOverview {
	user_id: string;
	user_label: string;
	teams: BudgetTeamSummary[];
	budgets: EffectiveBudget[];
	spent: string;
	remaining: string;
	decision: 'block' | 'warn' | 'allow';
	blocked_model_ids: string[];
}

export interface TeamBudgetAssignmentOverview {
	assignment_id: string;
	budget: Budget;
	spent: string;
	remaining: string;
	window_start: string;
	resets_at: string | null;
	affected_users: number;
	exhausted_users: number;
}

export interface TeamBudgetOverview {
	team_id: string;
	team_name: string;
	member_count: number;
	budgets: TeamBudgetAssignmentOverview[];
	spent: string;
	remaining: string;
	exhausted_count: number;
}

export interface BudgetResetEvent {
	id: string;
	assignment_id: string | null;
	budget_id: string | null;
	budget_name: string | null;
	team_id: string | null;
	team_name: string | null;
	user_id: string | null;
	user_label: string | null;
	kind: 'pooled' | 'per_user' | null;
	reason: string | null;
	reset_at: string;
	created_by: string | null;
	created_by_label: string | null;
}

export interface BudgetResetPayload {
	assignment_id?: string | null;
	budget_id?: string | null;
	team_id?: string | null;
	user_id?: string | null;
	kind?: 'pooled' | 'per_user' | null;
	reason?: string | null;
}
