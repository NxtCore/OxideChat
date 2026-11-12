import {defineStore} from 'pinia';
import type {
	AnalyticsDayModelRow,
	AnalyticsRow,
	ApiKeyAnalyticsRow,
	Budget,
	BudgetPayload,
	BudgetResetEvent,
	BudgetResetPayload,
	TeamBudgetOverview,
	UserBudgetOverview,
	UserBudgetStatus,
} from '~/types/budgets';
import type {PaginatedResponse} from '~/types/chat';

const PINNED_BUDGET_KEY = 'oxide-budget-pinned';

function loadBudgetPinned(): boolean {
	if (typeof window === 'undefined') return false;
	try {
		return localStorage.getItem(PINNED_BUDGET_KEY) === 'true';
	} catch {
		return false;
	}
}

export const useBudgetStore = defineStore('budget', {
	state: () => ({
		myStatus: null as UserBudgetStatus | null,
		myStatusStale: false,
		pinned: loadBudgetPinned(),
		budgets: [] as Budget[],
		analytics: {
			byModel: [] as AnalyticsRow[],
			byUser: [] as AnalyticsRow[],
			byTeam: [] as AnalyticsRow[],
			byApiKey: [] as ApiKeyAnalyticsRow[],
			byDay: [] as AnalyticsRow[],
			byDayModel: [] as AnalyticsDayModelRow[],
		},
		myAnalytics: {
			byModel: [] as AnalyticsRow[],
			byDay: [] as AnalyticsRow[],
			byDayModel: [] as AnalyticsDayModelRow[],
		},
		userAnalytics: {
			byModel: [] as AnalyticsRow[],
			byDay: [] as AnalyticsRow[],
			byDayModel: [] as AnalyticsDayModelRow[],
		},
		userOverview: [] as UserBudgetOverview[],
		teamOverview: [] as TeamBudgetOverview[],
		resetHistory: [] as BudgetResetEvent[],
		loading: false,
	}),
	getters: {
		lowestRemaining(state): number | null {
			if (!state.myStatus?.budgets.length) return null;
			return Math.min(...state.myStatus.budgets.map(budget => Number(budget.remaining)));
		},
		highestUsagePercent(state): number {
			if (!state.myStatus?.budgets.length) return 0;
			return Math.max(
				...state.myStatus.budgets.map((budget) => {
					const amount = Number(budget.amount);
					return amount > 0 ? Math.min(100, (Number(budget.spent) / amount) * 100) : 0;
				}),
			);
		},
	},
	actions: {
		togglePinned() {
			this.pinned = !this.pinned;
			try {
				localStorage.setItem(PINNED_BUDGET_KEY, String(this.pinned));
			} catch {}
		},
		async fetchMyBudget() {
			const {$customFetch} = useNuxtApp();
			this.myStatus = await $customFetch<UserBudgetStatus>('/api/v1/me/budget');
			this.myStatusStale = false;
			return this.myStatus;
		},
		async ensureMyBudget() {
			if (!this.myStatus || this.myStatusStale) await inflight('my-budget', () => this.fetchMyBudget());
		},
		async fetchBudgets() {
			const {$customFetch} = useNuxtApp();
			const response = await $customFetch<PaginatedResponse<Budget>>('/api/v1/admin/budgets?size=100');
			this.budgets = response?.items ?? [];
			return this.budgets;
		},
		async createBudget(payload: BudgetPayload) {
			const {$customFetch} = useNuxtApp();
			const budget = await $customFetch<Budget>('/api/v1/admin/budgets', {method: 'POST', body: payload});
			await this.fetchBudgets();
			return budget;
		},
		async updateBudget(id: string, payload: Partial<BudgetPayload>) {
			const {$customFetch} = useNuxtApp();
			const budget = await $customFetch<Budget>(`/api/v1/admin/budgets/${id}`, {method: 'PATCH', body: payload});
			await this.fetchBudgets();
			return budget;
		},
		async deleteBudget(id: string) {
			const {$customFetch} = useNuxtApp();
			await $customFetch(`/api/v1/admin/budgets/${id}`, {method: 'DELETE'});
			await this.fetchBudgets();
		},
		async fetchUserOverview() {
			const {$customFetch} = useNuxtApp();
			this.userOverview = await $customFetch<UserBudgetOverview[]>('/api/v1/admin/budgets/overview/users');
			return this.userOverview;
		},
		async fetchTeamOverview() {
			const {$customFetch} = useNuxtApp();
			this.teamOverview = await $customFetch<TeamBudgetOverview[]>('/api/v1/admin/budgets/overview/teams');
			return this.teamOverview;
		},
		async fetchResetHistory() {
			const {$customFetch} = useNuxtApp();
			this.resetHistory = await $customFetch<BudgetResetEvent[]>('/api/v1/admin/budgets/resets');
			return this.resetHistory;
		},
		async resetBudget(payload: BudgetResetPayload) {
			const {$customFetch} = useNuxtApp();
			const reset = await $customFetch<BudgetResetEvent>('/api/v1/admin/budgets/resets', {method: 'POST', body: payload});
			await Promise.all([this.fetchUserOverview(), this.fetchTeamOverview(), this.fetchResetHistory()]);
			return reset;
		},
		async fetchAllAnalytics(params: {from?: string; to?: string} = {}) {
			const {$customFetch} = useNuxtApp();
			const [byModel, byUser, byTeam, byApiKey, byDay, byDayModel] = await Promise.all([
				$customFetch<AnalyticsRow[]>('/api/v1/admin/analytics', {params: {...params, group_by: 'model'}}),
				$customFetch<AnalyticsRow[]>('/api/v1/admin/analytics', {params: {...params, group_by: 'user'}}),
				$customFetch<AnalyticsRow[]>('/api/v1/admin/analytics', {params: {...params, group_by: 'team'}}),
				$customFetch<ApiKeyAnalyticsRow[]>('/api/v1/admin/analytics', {params: {...params, group_by: 'api_key'}}),
				$customFetch<AnalyticsRow[]>('/api/v1/admin/analytics', {params: {...params, group_by: 'day'}}),
				$customFetch<AnalyticsDayModelRow[]>('/api/v1/admin/analytics', {params: {...params, group_by: 'day_model'}}),
			]);
			this.analytics.byModel = byModel ?? [];
			this.analytics.byUser = byUser ?? [];
			this.analytics.byTeam = byTeam ?? [];
			this.analytics.byApiKey = byApiKey ?? [];
			this.analytics.byDay = byDay ?? [];
			this.analytics.byDayModel = byDayModel ?? [];
		},
		async fetchUserAnalytics(userId: string, params: {from?: string; to?: string} = {}) {
			const {$customFetch} = useNuxtApp();
			const [byModel, byDay, byDayModel] = await Promise.all([
				$customFetch<AnalyticsRow[]>('/api/v1/admin/analytics', {params: {...params, group_by: 'model', user_id: userId}}),
				$customFetch<AnalyticsRow[]>('/api/v1/admin/analytics', {params: {...params, group_by: 'day', user_id: userId}}),
				$customFetch<AnalyticsDayModelRow[]>('/api/v1/admin/analytics', {params: {...params, group_by: 'day_model', user_id: userId}}),
			]);
			this.userAnalytics.byModel = byModel ?? [];
			this.userAnalytics.byDay = byDay ?? [];
			this.userAnalytics.byDayModel = byDayModel ?? [];
		},
		async fetchMyAnalytics(params: {from?: string; to?: string} = {}) {
			const {$customFetch} = useNuxtApp();
			const [byModel, byDay, byDayModel] = await Promise.all([
				$customFetch<AnalyticsRow[]>('/api/v1/me/analytics', {params: {...params, group_by: 'model'}}),
				$customFetch<AnalyticsRow[]>('/api/v1/me/analytics', {params: {...params, group_by: 'day'}}),
				$customFetch<AnalyticsDayModelRow[]>('/api/v1/me/analytics', {params: {...params, group_by: 'day_model'}}),
			]);
			this.myAnalytics.byModel = byModel ?? [];
			this.myAnalytics.byDay = byDay ?? [];
			this.myAnalytics.byDayModel = byDayModel ?? [];
		},
	},
});
