<template>
	<div class="space-y-6">
		<div>
			<h3 class="text-base font-semibold text-foreground mb-1">
				{{ t('parameters') }}
			</h3>
			<p class="text-xs text-muted-foreground">
				{{ t('parameters_desc') }}
			</p>
		</div>

		<div v-for="group in groups" :key="group.label" class="space-y-3">
			<p class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
				{{ t(group.label) }}
			</p>
			<div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
				<ParamCard
					v-for="p in group.params"
					:key="p.key"
					v-model="form.sampling[p.key]"
					v-bind="cardProps(p)"
					@toggle="toggle(form.sampling, p.key, p.def)"
				/>
			</div>
		</div>

		<div v-if="hasEffortReasoning || hasBudgetReasoning" class="space-y-3">
			<p class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
				{{ t('param_group_reasoning') }}
			</p>
			<div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
				<ParamCard
					v-if="hasEffortReasoning"
					:label="t('reasoning_effort')"
					:tooltip="t('reasoning_effort_tooltip')"
					:enabled="form.reasoning_effort !== null"
					@toggle="toggle(form, 'reasoning_effort', availableEffortLevels[1] ?? availableEffortLevels[0] ?? 'medium')"
				>
					<ShadSelect v-model="form.reasoning_effort">
						<ShadSelectTrigger class="h-8 text-xs w-full">
							<ShadSelectValue :placeholder="t('reasoning_effort_placeholder')" />
						</ShadSelectTrigger>
						<ShadSelectContent>
							<ShadSelectGroup>
								<ShadSelectItem v-for="effort in availableEffortLevels" :key="effort" :value="effort" class="text-xs">
									{{ effortLabel(effort) }}
								</ShadSelectItem>
							</ShadSelectGroup>
						</ShadSelectContent>
					</ShadSelect>
				</ParamCard>

				<ParamCard
					v-if="hasBudgetReasoning"
					v-model="form.reasoning_budget_tokens"
					:label="t('reasoning_budget')"
					:tooltip="t('reasoning_budget_tooltip')"
					:enabled="form.reasoning_budget_tokens !== null"
					:display="`${Math.round((form.reasoning_budget_tokens ?? 0) / 1024)}K`"
					:min="reasoningBudgetRange.min"
					:max="reasoningBudgetRange.max"
					:step="1024"
					:labels="[`${Math.round(reasoningBudgetRange.min / 1024)}K`, `${Math.round(reasoningBudgetRange.max / 1024)}K`]"
					@toggle="toggle(form, 'reasoning_budget_tokens', reasoningBudgetRange.min)"
				/>
			</div>
		</div>

		<div class="space-y-3">
			<p class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
				{{ t('param_group_advanced') }}
			</p>
			<div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
				<ParamCard :label="t('seed')" :tooltip="t('seed_tooltip')" :enabled="form.sampling.seed !== null" @toggle="toggle(form.sampling, 'seed', 42)">
					<ShadInput
						v-model.number="form.sampling.seed"
						type="number"
						min="0"
						step="1"
						class="h-8 text-xs font-mono"
						:placeholder="t('seed_placeholder')"
					/>
				</ParamCard>

				<ParamCard
					:label="t('stop_sequences')"
					:tooltip="t('stop_sequences_tooltip')"
					:enabled="form.sampling.stop !== null"
					@toggle="toggle(form.sampling, 'stop', [])"
				>
					<div v-if="form.sampling.stop?.length > 0" class="flex flex-wrap gap-1">
						<div v-for="(seq, idx) in form.sampling.stop" :key="idx" class="flex items-center gap-1 rounded-md bg-muted px-2 py-0.5 text-xs font-mono">
							<span>{{ seq }}</span>
							<button class="text-muted-foreground hover:text-foreground transition-colors" @click="form.sampling.stop.splice(idx, 1)">
								<X class="h-3 w-3" />
							</button>
						</div>
					</div>
					<div class="flex gap-1.5">
						<ShadInput
							v-model="newStopSequence"
							class="h-8 text-xs font-mono flex-1"
							:placeholder="t('stop_sequences_placeholder')"
							@keydown.enter.prevent="addStopSequence"
						/>
						<ShadButton variant="outline" size="icon" class="h-8 w-8 flex-shrink-0" :disabled="!newStopSequence.trim()" @click="addStopSequence">
							<Plus class="h-3.5 w-3.5" />
						</ShadButton>
					</div>
				</ParamCard>

				<ParamCard
					v-for="key in ['parallel_tool_calls', 'logprobs']"
					:key="key"
					:label="t(key)"
					:tooltip="t(`${key}_tooltip`)"
					:enabled="form.sampling[key] !== null"
					@toggle="toggle(form.sampling, key, key === 'parallel_tool_calls')"
				>
					<template #extra>
						<ShadSwitch v-if="form.sampling[key] !== null" v-model:model-value="form.sampling[key]" />
					</template>
				</ParamCard>

				<ParamCard
					v-if="form.sampling.logprobs"
					v-model="form.sampling.top_logprobs"
					v-bind="cardProps(topLogprobs)"
					@toggle="toggle(form.sampling, 'top_logprobs', topLogprobs.def)"
				/>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import {Plus, X} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import ParamCard from './ParamCard.vue';

type SliderParam = {key: string; def: number; min: number; max: number; step: number; show: (v: number) => string; labels: string[]};

const props = defineProps<{
	form: Record<string, any>;
	capabilities: string[];
}>();

const store = useMainStore();
const newStopSequence = ref('');

const t = (key: string) => store.getTranslation(`settings.models.editor.${key}`);
const plain = (v: number) => String(v);
const fixed = (v: number) => v.toFixed(2);
const signed = (v: number) => (v >= 0 ? '+' : '') + v.toFixed(2);
const penalty = {min: -2, max: 2, step: 0.01, show: signed, labels: ['-2', '0', '+2']};

const topLogprobs: SliderParam = {key: 'top_logprobs', def: 5, min: 0, max: 20, step: 1, show: plain, labels: ['0', '20']};

const groups = computed<{label: string; params: SliderParam[]}[]>(() => [
	{
		label: 'param_group_core',
		params: [
			{key: 'temperature', def: 0.7, min: 0, max: 2, step: 0.01, show: fixed, labels: [`0 — ${t('temperature_label_low')}`, `${t('temperature_label_high')} — 2`]},
			{key: 'max_tokens', def: 4096, min: 256, max: 128000, step: 256, show: v => v.toLocaleString(), labels: ['256', '128 000']},
		],
	},
	{
		label: 'param_group_token',
		params: [
			{key: 'top_p', def: 1, min: 0, max: 1, step: 0.01, show: fixed, labels: ['0', '1']},
			{key: 'top_k', def: 40, min: 1, max: 500, step: 1, show: plain, labels: ['1', '500']},
		],
	},
	{
		label: 'param_group_penalties',
		params: [
			{key: 'frequency_penalty', def: 0, ...penalty},
			{key: 'presence_penalty', def: 0, ...penalty},
		],
	},
]);

function cardProps(p: SliderParam) {
	const value: number | null = props.form.sampling[p.key];
	return {
		label: t(p.key),
		tooltip: t(`${p.key}_tooltip`),
		enabled: value !== null,
		display: value === null ? undefined : p.show(value),
		min: p.min,
		max: p.max,
		step: p.step,
		labels: p.labels,
	};
}

const hasEffortReasoning = computed(() => props.capabilities.some(c => c.startsWith('REASONING_EFFORT_')));
const hasBudgetReasoning = computed(() => props.capabilities.some(c => c.startsWith('REASONING_BUDGET_TOKENS_')));

const availableEffortLevels = computed(() =>
	['none', 'minimal', 'low', 'medium', 'high', 'xhigh'].filter(e => props.capabilities.includes(`REASONING_EFFORT_${e.toUpperCase()}`)),
);

const reasoningBudgetRange = computed(() => {
	for (const cap of props.capabilities) {
		const match = cap.match(/REASONING_BUDGET_TOKENS_(\d+)_(\d+)/);
		if (match) return {min: parseInt(match[1] ?? '0'), max: parseInt(match[2] ?? '0')};
	}
	return {min: 1024, max: 32768};
});

function effortLabel(effort: string): string {
	return store.getTranslation(`chat.reasoning_selector.${effort === 'xhigh' ? 'extra_high' : effort}`);
}

function toggle(target: Record<string, any>, key: string, defaultValue: any) {
	target[key] = target[key] !== null ? null : defaultValue;
}

function addStopSequence() {
	const seq = newStopSequence.value.trim();
	if (!seq) return;
	if (!props.form.sampling.stop) props.form.sampling.stop = [];
	if (!props.form.sampling.stop.includes(seq)) props.form.sampling.stop.push(seq);
	newStopSequence.value = '';
}
</script>
