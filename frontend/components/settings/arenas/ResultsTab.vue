<template>
	<div class="space-y-3">
		<div>
			<h3 class="text-base font-semibold text-foreground mb-1">{{ t('results_title') }}</h3>
			<p class="text-xs text-muted-foreground">{{ t('results_description') }}</p>
		</div>
		<div v-if="!rows" class="flex justify-center py-8 text-muted-foreground">
			<Loader2 class="h-5 w-5 animate-spin" />
		</div>
		<p v-else-if="rows.length === 0" class="rounded-lg border border-dashed border-border px-4 py-8 text-center text-sm text-muted-foreground" data-testid="arena-results-empty">
			{{ t('results_empty') }}
		</p>
		<table v-else class="w-full text-sm" data-testid="arena-results">
			<thead class="text-left text-xs text-muted-foreground">
				<tr>
					<th class="py-2 font-medium">{{ t('results_model') }}</th>
					<th class="py-2 text-right font-medium">{{ t('results_rounds') }}</th>
					<th class="py-2 text-right font-medium">{{ t('results_wins') }}</th>
					<th class="py-2 text-right font-medium">{{ t('results_win_rate') }}</th>
				</tr>
			</thead>
			<tbody>
				<tr v-for="row in rows" :key="row.model_id" class="border-t border-border" data-testid="arena-results-row">
					<td class="py-2 text-foreground">{{ row.model_name }}</td>
					<td class="py-2 text-right tabular-nums" data-testid="arena-results-rounds">{{ row.rounds }}</td>
					<td class="py-2 text-right tabular-nums" data-testid="arena-results-wins">{{ row.wins }}</td>
					<td class="py-2 text-right tabular-nums">{{ Math.round(row.win_rate * 100) }}%</td>
				</tr>
			</tbody>
		</table>
	</div>
</template>

<script setup lang="ts">
import {Loader2} from 'lucide-vue-next';
import {useMainStore} from '~/stores';
import type {ArenaResultRow} from '~/types/presets';

const props = defineProps<{presetId: string}>();

const store = useMainStore();
const t = (key: string) => store.getTranslation(`presets.arena.${key}`);
const rows = ref<ArenaResultRow[] | null>(null);

onMounted(async () => {
	try {
		const {$customFetch} = useNuxtApp();
		rows.value = await $customFetch<ArenaResultRow[]>(`${presetsPath}/${props.presetId}/arena-results`);
	} catch (e: any) {
		rows.value = [];
		store.toast(t('results_error'), {type: 'error', description: errorMessage(e, '')});
	}
});
</script>
