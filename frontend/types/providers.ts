export type ProviderBillingStatus = 'NOT_CONFIGURED' | 'AVAILABLE' | 'UNSUPPORTED' | 'UNAUTHORIZED' | 'UPSTREAM_ERROR';
export type ProviderBillingMetricKind = 'CREDIT_BALANCE' | 'KEY_LIMIT' | 'SPEND_THRESHOLD' | 'SPEND_ONLY';

export interface ProviderBillingMetric {
	metric_kind: ProviderBillingMetricKind;
	currency: string;
	period_start: string | null;
	period_end: string | null;
	limit_amount: string | number | null;
	spent_amount: string | number | null;
	remaining_amount: string | number | null;
	is_hard_limit: boolean;
	thresholds: Array<string | number>;
}

export interface ProviderBillingOverview {
	provider_id: string;
	provider_kind: string;
	status: ProviderBillingStatus;
	is_enabled: boolean;
	has_billing_credential: boolean;
	external_scope_id: string | null;
	external_scope_name: string | null;
	upstream: ProviderBillingMetric | null;
	local: {
		currency: string;
		period_start: string;
		period_end: string;
		spent_amount: string | number;
	};
	last_synced_at: string | null;
	is_stale: boolean;
	error_code: string | null;
}

export interface UpdateProviderBillingPayload {
	is_enabled: boolean;
	credential?: string;
	external_scope_id: string | null;
	external_scope_name: string | null;
}
