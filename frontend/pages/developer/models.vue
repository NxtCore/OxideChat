<template>
	<div class="w-full space-y-4">
		<div>
			<h2 class="text-lg font-semibold text-foreground">{{ store.getTranslation('developer.models.title') }}</h2>
			<p class="text-sm text-muted-foreground">{{ store.getTranslation('developer.models.description') }}</p>
		</div>

		<div v-if="developer.error" class="flex items-center justify-between gap-3 rounded-lg border border-border bg-card p-4">
			<p class="text-sm text-foreground">{{ store.getTranslation('developer.errors.load') }}</p>
			<ShadButton variant="outline" size="sm" @click="load">{{ store.getTranslation('common.refresh') }}</ShadButton>
		</div>

		<div v-if="!developer.modelsLoaded && !developer.error" class="space-y-2">
			<ShadSkeleton v-for="n in 6" :key="n" class="h-12 w-full" />
		</div>

		<p v-else-if="developer.modelsLoaded && !developer.models.length" class="rounded-lg border border-dashed border-border bg-card py-10 text-center text-sm text-muted-foreground">
			{{ store.getTranslation(developer.teams.length ? 'developer.no_models' : 'developer.models.no_teams') }}
		</p>

		<template v-else-if="developer.modelsLoaded">
			<div class="space-y-2">
				<div class="flex flex-col gap-2 md:flex-row md:items-center">
					<div class="relative min-w-0 flex-1">
						<Search class="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
						<ShadInput v-model="query" class="pl-9" autocomplete="off" data-testid="model-search" :placeholder="store.getTranslation('developer.model_search.placeholder')" :aria-label="store.getTranslation('developer.model_search.placeholder')" />
					</div>
					<ShadSelect v-if="providers.length > 1" v-model="providerFilter">
						<ShadSelectTrigger class="w-full md:w-44" :aria-label="store.getTranslation('developer.models.provider')"><ShadSelectValue /></ShadSelectTrigger>
						<ShadSelectContent>
							<ShadSelectItem value="all">{{ store.getTranslation('developer.models.all_providers') }}</ShadSelectItem>
							<ShadSelectItem v-for="provider in providers" :key="provider" :value="provider">{{ provider }}</ShadSelectItem>
						</ShadSelectContent>
					</ShadSelect>
					<ShadSelect v-if="developer.teams.length > 1" v-model="teamFilter">
						<ShadSelectTrigger class="w-full md:w-44" :aria-label="store.getTranslation('developer.models.team')"><ShadSelectValue /></ShadSelectTrigger>
						<ShadSelectContent>
							<ShadSelectItem value="all">{{ store.getTranslation('developer.models.all_teams') }}</ShadSelectItem>
							<ShadSelectItem v-for="team in developer.teams" :key="team.id" :value="team.id">{{ team.name }}</ShadSelectItem>
						</ShadSelectContent>
					</ShadSelect>
				</div>
				<div class="flex flex-wrap items-center gap-1.5">
					<button
						v-for="feature in FEATURES"
						:key="feature.id"
						type="button"
						class="inline-flex items-center gap-1.5 rounded-md border px-2.5 py-1 text-xs transition-colors"
						:class="active.has(feature.id) ? 'border-primary bg-primary/10 text-foreground' : 'border-border text-muted-foreground hover:bg-accent/50 hover:text-foreground'"
						:aria-pressed="active.has(feature.id)"
						:data-testid="`model-filter-${feature.id}`"
						@click="toggleFeature(feature.id)"
					>
						<component :is="feature.icon" class="h-3.5 w-3.5" />{{ store.getTranslation(feature.label) }}
					</button>
					<span class="ml-auto text-sm tabular-nums text-muted-foreground" data-testid="model-count">{{ store.getTranslation('developer.models_count', {count: visible.length}) }}</span>
				</div>
			</div>

			<p v-if="!visible.length" class="rounded-lg border border-dashed border-border bg-card py-10 text-center text-sm text-muted-foreground">
				{{ store.getTranslation('developer.model_search.empty') }}
			</p>
			<div v-else class="overflow-x-auto rounded-lg border border-border bg-card">
				<table class="w-full min-w-[760px] text-sm">
					<thead>
						<tr class="border-b border-border text-left text-xs text-muted-foreground">
							<th v-for="column in COLUMNS" :key="column.id" class="px-3 py-2 font-medium" :class="column.numeric ? 'text-right' : ''" :aria-sort="sortBy === column.id ? (sortDescending ? 'descending' : 'ascending') : undefined">
								<button type="button" class="inline-flex items-center gap-1 hover:text-foreground" :data-testid="`model-sort-${column.id}`" @click="sort(column.id)">
									{{ store.getTranslation(column.label) }}
									<component :is="sortDescending ? ArrowDown : ArrowUp" v-if="sortBy === column.id" class="h-3 w-3" />
								</button>
							</th>
							<th class="px-3 py-2 font-medium">{{ store.getTranslation('developer.models.features') }}</th>
							<th class="px-3 py-2" />
						</tr>
					</thead>
					<tbody>
						<tr v-for="model in visible" :key="model.id" class="border-b border-border/60 last:border-b-0" :data-testid="`model-row-${model.id}`">
							<td class="max-w-80 px-3 py-2">
								<p class="truncate font-medium text-foreground" :title="model.display_name">{{ model.display_name }}</p>
								<p class="truncate font-mono text-xs text-muted-foreground" :title="model.id">{{ model.id }}</p>
							</td>
							<td class="whitespace-nowrap px-3 py-2 text-muted-foreground">{{ model.provider }}</td>
							<td class="whitespace-nowrap px-3 py-2 text-right tabular-nums text-muted-foreground">{{ formatContext(model.context_length) }}</td>
							<td class="whitespace-nowrap px-3 py-2 text-right tabular-nums text-muted-foreground">{{ formatPrice(model.price_input) }}</td>
							<td class="whitespace-nowrap px-3 py-2 text-right tabular-nums text-muted-foreground">{{ formatPrice(model.price_output) }}</td>
							<td class="px-3 py-2">
								<div class="flex gap-1.5 text-muted-foreground">
									<component :is="feature.icon" v-for="feature in FEATURES.filter(entry => entry.matches(model))" :key="feature.id" class="h-3.5 w-3.5" :aria-label="store.getTranslation(feature.label)" />
								</div>
							</td>
							<td class="px-2 py-2">
								<div class="flex justify-end gap-0.5">
									<ShadButton variant="ghost" size="icon" class="h-7 w-7" :aria-label="store.getTranslation('developer.models.copy_id')" :title="store.getTranslation('developer.models.copy_id')" @click="store.copyToClipboard(model.id)">
										<Copy class="h-3.5 w-3.5" />
									</ShadButton>
									<ShadButton as-child variant="ghost" size="icon" class="h-7 w-7" :title="store.getTranslation('developer.models.try')">
										<NuxtLink :to="{path: '/developer/playground', query: {model: model.id}}" :aria-label="store.getTranslation('developer.models.try')">
											<FlaskConical class="h-3.5 w-3.5" />
										</NuxtLink>
									</ShadButton>
								</div>
							</td>
						</tr>
					</tbody>
				</table>
			</div>
			<p class="text-xs text-muted-foreground">{{ store.getTranslation('developer.models.price_hint') }}</p>
		</template>
	</div>
</template>

<script setup lang="ts">
import {ArrowDown, ArrowUp, Copy, Eye, FlaskConical, Image, Search, Sparkles, Wrench} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {useDeveloperStore} from '@/stores/developerStore';
import type {DeveloperModel} from '~/types/developer';

type SortColumn = 'name' | 'provider' | 'context' | 'input' | 'output';

const FEATURES = [
	{id: 'tools', label: 'developer.models.filter_tools', icon: Wrench, matches: (model: DeveloperModel) => model.capabilities.includes('TOOLS')},
	{id: 'reasoning', label: 'developer.models.filter_reasoning', icon: Sparkles, matches: (model: DeveloperModel) => model.capabilities.some(capability => capability === 'REASONING' || capability.startsWith('REASONING_'))},
	{id: 'vision', label: 'developer.models.filter_vision', icon: Eye, matches: (model: DeveloperModel) => model.input_modalities.includes('IMAGE')},
	{id: 'image', label: 'developer.models.filter_image', icon: Image, matches: (model: DeveloperModel) => model.output_modalities.includes('IMAGE')},
];

const COLUMNS: {id: SortColumn; label: string; numeric?: boolean}[] = [
	{id: 'name', label: 'developer.models.name'},
	{id: 'provider', label: 'developer.models.provider'},
	{id: 'context', label: 'developer.models.context', numeric: true},
	{id: 'input', label: 'developer.models.input', numeric: true},
	{id: 'output', label: 'developer.models.output', numeric: true},
];

const store = useMainStore();
const developer = useDeveloperStore();

const query = ref('');
const providerFilter = ref('all');
const teamFilter = ref('all');
const active = ref(new Set<string>());
const sortBy = ref<SortColumn>('name');
const sortDescending = ref(false);

const providers = computed(() => [...new Set(developer.models.map(model => model.provider))].sort((a, b) => a.localeCompare(b)));

function knownPrice(value: string | null): number | null {
	const price = value === null ? null : Number(value);
	return price === null || Number.isNaN(price) || price < 0 ? null : price;
}

function sortValue(model: DeveloperModel, column: SortColumn): string | number | null {
	if (column === 'name') return model.display_name;
	if (column === 'provider') return model.provider;
	if (column === 'context') return model.context_length;
	return knownPrice(column === 'input' ? model.price_input : model.price_output);
}

const visible = computed(() => {
	const needle = query.value.trim().toLowerCase();
	const features = FEATURES.filter(feature => active.value.has(feature.id));
	const direction = sortDescending.value ? -1 : 1;
	return developer.models
		.filter(model => !needle || `${model.display_name} ${model.id}`.toLowerCase().includes(needle))
		.filter(model => providerFilter.value === 'all' || model.provider === providerFilter.value)
		.filter(model => teamFilter.value === 'all' || model.team_ids.includes(teamFilter.value))
		.filter(model => features.every(feature => feature.matches(model)))
		.sort((a, b) => {
			const left = sortValue(a, sortBy.value);
			const right = sortValue(b, sortBy.value);
			if (left === null || right === null) return left === right ? 0 : left === null ? 1 : -1;
			if (typeof left === 'number' && typeof right === 'number') return (left - right) * direction;
			return String(left).localeCompare(String(right), undefined, {numeric: true}) * direction;
		});
});

function toggleFeature(id: string) {
	const next = new Set(active.value);
	if (next.has(id)) next.delete(id);
	else next.add(id);
	active.value = next;
}

function sort(column: SortColumn) {
	sortDescending.value = sortBy.value === column && !sortDescending.value;
	sortBy.value = column;
}

function formatContext(value: number | null): string {
	if (!value) return '–';
	return value >= 1_000_000 ? `${Number((value / 1_000_000).toFixed(1))}M` : `${Math.round(value / 1000)}k`;
}

function formatPrice(value: string | null): string {
	const price = knownPrice(value);
	return price === null ? '–' : `$${Number(price.toFixed(4))}`;
}

function load() {
	developer.load().catch(() => {});
	developer.loadModels().catch(() => {});
}

onMounted(load);
</script>
