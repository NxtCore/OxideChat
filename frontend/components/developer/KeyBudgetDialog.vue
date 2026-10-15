<template>
	<ShadDialog :open="open" @update:open="emit('update:open', $event)">
		<ShadDialogContent class="max-h-[90dvh] overflow-y-auto sm:max-w-[480px]">
			<ShadDialogHeader>
				<ShadDialogTitle>{{ store.getTranslation('developer.budget.title') }}</ShadDialogTitle>
				<ShadDialogDescription>{{ store.getTranslation('developer.budget.description', {name: apiKey?.name ?? ''}) }}</ShadDialogDescription>
			</ShadDialogHeader>
			<div v-if="loading" class="flex justify-center py-8 text-muted-foreground"><Loader2 class="h-5 w-5 animate-spin" /></div>
			<div v-else class="space-y-4 py-2">
				<div v-if="current" class="space-y-2 rounded-md border border-border bg-muted/30 p-3" data-testid="budget-status">
					<div class="flex items-center justify-between text-sm">
						<span class="text-muted-foreground">{{ store.getTranslation('developer.budget.spent') }}</span>
						<span class="font-medium tabular-nums text-foreground">{{ formatMoney(current.spent) }} / {{ formatMoney(current.amount) }}</span>
					</div>
					<div class="h-1.5 rounded-full bg-muted">
						<div class="h-1.5 rounded-full" :class="current.exhausted ? 'bg-destructive' : 'bg-primary'" :style="{width: `${percent}%`}" />
					</div>
					<div class="flex items-center justify-between text-xs text-muted-foreground">
						<span>{{ store.getTranslation('developer.budget.remaining', {amount: formatMoney(current.remaining)}) }}</span>
						<span>
							{{ current.resets_at ? store.getTranslation('developer.budget.resets', {date: new Date(current.resets_at).toLocaleString(store.base?.language ?? 'en')}) : store.getTranslation('developer.budget.no_reset') }}
						</span>
					</div>
					<p v-if="current.exhausted" class="text-xs text-destructive">{{ store.getTranslation('developer.budget.exhausted') }}</p>
				</div>
				<p v-else class="rounded-md border border-dashed border-border p-3 text-center text-sm text-muted-foreground">{{ store.getTranslation('developer.budget.none') }}</p>

				<div class="space-y-1.5">
					<ShadLabel for="budget-amount">{{ store.getTranslation('developer.budget.amount') }}</ShadLabel>
					<ShadInput id="budget-amount" v-model="form.amount" type="number" min="0" step="0.01" />
				</div>
				<div class="grid gap-4 sm:grid-cols-2">
					<div class="space-y-1.5">
						<ShadLabel>{{ store.getTranslation('developer.budget.interval') }}</ShadLabel>
						<ShadSelect v-model="form.interval">
							<ShadSelectTrigger class="w-full"><ShadSelectValue /></ShadSelectTrigger>
							<ShadSelectContent>
								<ShadSelectItem v-for="interval in intervals" :key="interval" :value="interval">{{ store.getTranslation(`developer.budget.interval_${interval}`) }}</ShadSelectItem>
							</ShadSelectContent>
						</ShadSelect>
					</div>
					<div class="space-y-1.5">
						<ShadLabel>{{ store.getTranslation('developer.budget.on_exceed') }}</ShadLabel>
						<ShadSelect v-model="form.on_exceed">
							<ShadSelectTrigger class="w-full"><ShadSelectValue /></ShadSelectTrigger>
							<ShadSelectContent>
								<ShadSelectItem value="block">{{ store.getTranslation('developer.budget.on_exceed_block') }}</ShadSelectItem>
								<ShadSelectItem value="warn">{{ store.getTranslation('developer.budget.on_exceed_warn') }}</ShadSelectItem>
							</ShadSelectContent>
						</ShadSelect>
					</div>
				</div>
				<p class="text-xs text-muted-foreground">{{ store.getTranslation('developer.budget.rotation_hint') }}</p>
			</div>
			<ShadDialogFooter class="gap-2">
				<ShadButton v-if="current" variant="outline" class="text-destructive hover:text-destructive" :disabled="saving" @click="clear">
					{{ store.getTranslation('developer.budget.clear') }}
				</ShadButton>
				<ShadButton variant="outline" @click="emit('update:open', false)">{{ store.getTranslation('common.cancel') }}</ShadButton>
				<ShadButton :disabled="loading || saving || !(Number(form.amount) > 0)" @click="save">
					<Loader2 v-if="saving" class="mr-2 h-4 w-4 animate-spin" />
					{{ store.getTranslation('common.save') }}
				</ShadButton>
			</ShadDialogFooter>
		</ShadDialogContent>
	</ShadDialog>
</template>

<script setup lang="ts">
import {Loader2} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {useDeveloperStore} from '@/stores/developerStore';
import type {BudgetInterval, BudgetOnExceed, DeveloperKey, DeveloperKeyBudget} from '~/types/developer';

const props = defineProps<{open: boolean; apiKey: DeveloperKey | null}>();
const emit = defineEmits<{'update:open': [value: boolean]}>();

const store = useMainStore();
const developer = useDeveloperStore();
const intervals: BudgetInterval[] = ['daily', 'weekly', 'monthly', 'total'];
const current = ref<DeveloperKeyBudget | null>(null);
const loading = ref(false);
const saving = ref(false);
const form = reactive({amount: '' as string | number, interval: 'monthly' as BudgetInterval, on_exceed: 'block' as BudgetOnExceed});

const percent = computed(() => {
	const amount = Number(current.value?.amount ?? 0);
	return amount > 0 ? Math.min(100, (Number(current.value?.spent ?? 0) / amount) * 100) : 0;
});

function applyCurrent(budget: DeveloperKeyBudget | null) {
	current.value = budget;
	form.amount = budget ? Number(budget.amount) : '';
	form.interval = budget?.interval ?? 'monthly';
	form.on_exceed = budget?.on_exceed ?? 'block';
}

watch(
	() => props.open,
	async (value) => {
		if (!value || !props.apiKey) return;
		loading.value = true;
		applyCurrent(null);
		try {
			applyCurrent(await developer.getBudget(props.apiKey.id));
		} catch (e: any) {
			store.toast(store.getTranslation('developer.budget.load_error'), {type: 'error', description: errorMessage(e, '')});
			emit('update:open', false);
		} finally {
			loading.value = false;
		}
	}
);

async function save() {
	if (!props.apiKey) return;
	saving.value = true;
	try {
		await developer.setBudget(props.apiKey.id, {amount: Number(form.amount), interval: form.interval, on_exceed: form.on_exceed});
		store.toast(store.getTranslation('developer.budget.saved'), {type: 'success'});
		emit('update:open', false);
	} catch (e: any) {
		store.toast(store.getTranslation('developer.budget.save_error'), {type: 'error', description: errorMessage(e, '')});
	} finally {
		saving.value = false;
	}
}

async function clear() {
	if (!props.apiKey) return;
	saving.value = true;
	try {
		await developer.clearBudget(props.apiKey.id);
		store.toast(store.getTranslation('developer.budget.cleared'), {type: 'success'});
		emit('update:open', false);
	} catch (e: any) {
		store.toast(store.getTranslation('developer.budget.save_error'), {type: 'error', description: errorMessage(e, '')});
	} finally {
		saving.value = false;
	}
}
</script>
