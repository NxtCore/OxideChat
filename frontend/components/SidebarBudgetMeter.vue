<template>
	<div>
		<div class="mb-1.5 flex items-center justify-between gap-2 text-xs text-muted-foreground">
			<span>{{ store.getTranslation('settings.tabs.budgets') }}</span>
			<div class="flex items-center gap-1">
				<span :class="budgetStore.highestUsagePercent >= 90 ? 'text-destructive' : ''">
					{{ store.getTranslation('sidebar.budget.remaining', {amount: formatMoney(budgetStore.lowestRemaining ?? 0)}) }}
				</span>
				<button
					type="button"
					class="rounded p-0.5 hover:bg-accent hover:text-accent-foreground"
					data-testid="budget-pin-toggle"
					:title="store.getTranslation(budgetStore.pinned ? 'sidebar.budget.unpin' : 'sidebar.budget.pin')"
					:aria-label="store.getTranslation(budgetStore.pinned ? 'sidebar.budget.unpin' : 'sidebar.budget.pin')"
					:aria-pressed="budgetStore.pinned"
					@click.stop.prevent="budgetStore.togglePinned()"
				>
					<PinOff v-if="budgetStore.pinned" class="h-3.5 w-3.5" />
					<Pin v-else class="h-3.5 w-3.5" />
				</button>
			</div>
		</div>
		<div class="h-1.5 rounded-full bg-muted">
			<div
				class="h-1.5 rounded-full transition-all"
				:class="budgetStore.highestUsagePercent >= 90 ? 'bg-destructive' : 'bg-primary'"
				:style="{width: `${budgetStore.highestUsagePercent}%`}"
			/>
		</div>
	</div>
</template>

<script setup lang="ts">
import {Pin, PinOff} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {useBudgetStore} from '@/stores/budgetStore';

const store = useMainStore();
const budgetStore = useBudgetStore();
</script>
