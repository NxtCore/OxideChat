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
