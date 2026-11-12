<template>
	<SettingsUsersSection
		:title="t('budgets_title')"
		:description="t('budgets_description')"
		to="/settings/budgets"
		:link-label="t('manage_budgets')"
		:empty-title="budgets.length ? undefined : t('budgets_empty')"
		:empty-hint="t('budgets_empty_hint')"
		test-id="user-budgets"
	>
		<ul class="divide-y divide-border overflow-hidden rounded-lg border border-border bg-card">
			<li v-for="budget in budgets" :key="budget.assignment_id">
				<NuxtLink
					:to="`/settings/budgets?budget=${budget.budget.id}`"
					class="block space-y-2 px-4 py-3 transition-colors hover:bg-accent/40 focus-visible:bg-accent/40 focus-visible:outline-none"
					data-testid="user-budget"
				>
					<span class="flex flex-wrap items-center gap-x-3 gap-y-1">
						<span class="truncate text-sm font-medium text-foreground">{{ budget.budget.name }}</span>
						<span class="rounded bg-muted px-1.5 py-0.5 text-xs text-muted-foreground">{{ budget.budget.kind }} · {{ budget.budget.interval }}</span>
						<span v-if="budget.exhausted" class="rounded bg-destructive/10 px-1.5 py-0.5 text-xs text-destructive">{{ t('budget_exhausted') }}</span>
						<span class="ml-auto text-sm tabular-nums text-foreground" data-testid="user-budget-spend">
							{{ t('budget_used', {spent: formatMoney(budget.spent), amount: formatMoney(budget.amount)}) }}
						</span>
					</span>
					<span class="block h-1.5 rounded-full bg-muted" role="progressbar" :aria-valuenow="percentOf(budget)" aria-valuemin="0" aria-valuemax="100" :aria-label="budget.budget.name">
						<span class="block h-1.5 rounded-full" :class="percentOf(budget) >= 90 ? 'bg-destructive' : 'bg-primary'" :style="{width: `${percentOf(budget)}%`}" />
					</span>
					<span class="flex flex-wrap justify-between gap-x-4 text-xs text-muted-foreground">
						<span data-testid="user-budget-source">{{ sourceOf(budget) }}</span>
						<span v-if="budget.resets_at">{{ t('budget_resets', {date: formatDateTime(budget.resets_at)}) }}</span>
					</span>
				</NuxtLink>
			</li>
		</ul>
	</SettingsUsersSection>
</template>

<script setup lang="ts">
import {useMainStore} from '~/stores';
import type {EffectiveBudget} from '~/types/budgets';
import type {UserTeamResponse} from '~/types/users';

const props = defineProps<{budgets: EffectiveBudget[]; teams: UserTeamResponse[]}>();

const store = useMainStore();
const t = (key: string, args?: Record<string, string>) => store.getTranslation(`settings.user_detail.${key}`, args);

function percentOf(budget: EffectiveBudget): number {
	const amount = Number(budget.amount);
	return amount > 0 ? Math.min(100, Math.round((Number(budget.spent) / amount) * 100)) : 100;
}

function sourceOf(budget: EffectiveBudget): string {
	const team = props.teams.find(member => member.id === budget.team_id);
	return team ? t('budget_source_team', {team: team.name}) : t('budget_source_user');
}
</script>
