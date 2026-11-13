<template>
	<ShadPopover v-model:open="open">
		<ShadPopoverTrigger as-child>
			<ShadButton
				variant="ghost"
				data-testid="sampling-selector"
				:class="cn('h-8 w-auto gap-1.5 px-2', activeCount > 0 ? 'text-primary' : 'text-muted-foreground')"
				:aria-label="t('title')"
			>
				<SlidersHorizontal class="h-4 w-4" />
				<span v-if="activeCount > 0" class="text-xs font-medium tabular-nums" data-testid="sampling-active-count">{{ activeCount }}</span>
				<span v-else class="text-xs">{{ store.getTranslation('chat.composer.default') }}</span>
			</ShadButton>
		</ShadPopoverTrigger>
		<ShadPopoverContent class="w-96 max-w-[calc(100vw-1rem)] max-h-[70vh] overflow-y-auto space-y-4" align="start" data-testid="sampling-panel">
			<div>
				<h3 class="text-sm font-semibold text-foreground">{{ t('title') }}</h3>
				<p class="text-xs text-muted-foreground">{{ t('description') }}</p>
				<p v-if="presetDefaults" class="mt-2 rounded-md bg-primary/10 px-2 py-1.5 text-xs text-primary" data-testid="sampling-preset-defaults">
					{{ store.getTranslation('presets.sampling.defaults', {preset: presetName, values: presetDefaults}) }}
				</p>
			</div>

			<div class="space-y-3">
				<ParamCard
					v-for="p in sliders"
					:key="p.key"
					v-model="form[p.key]"
					:data-testid="`sampling-param-${p.key}`"
					:label="editor(p.key)"
					:tooltip="editor(`${p.key}_tooltip`)"
					:enabled="form[p.key] !== null"
					:display="form[p.key] === null ? undefined : p.show(form[p.key] as number)"
					:min="p.min"
					:max="p.max"
					:step="p.step"
					:labels="p.labels"
					@toggle="toggle(p.key, presetValue(p.key) ?? p.def)"
				/>
				<ParamCard
					data-testid="sampling-param-seed"
					:label="editor('seed')"
					:tooltip="editor('seed_tooltip')"
					:enabled="form.seed !== null"
					@toggle="toggle('seed', presetValue('seed') ?? 42)"
				>
					<ShadInput v-model.number="form.seed" type="number" step="1" class="h-8 text-xs font-mono" :placeholder="editor('seed_placeholder')" />
				</ParamCard>
			</div>

			<div class="space-y-2">
				<div>
					<p class="text-sm font-medium text-foreground">{{ t('custom') }}</p>
					<p class="text-xs text-muted-foreground">{{ t('custom_description') }}</p>
				</div>
				<div v-for="(field, idx) in customFields" :key="idx" class="flex gap-1.5" data-testid="sampling-custom-field">
					<ShadInput v-model="field.key" class="h-8 text-xs font-mono w-2/5" :placeholder="t('custom_key')" data-testid="sampling-custom-key" />
					<ShadInput v-model="field.value" class="h-8 text-xs font-mono flex-1" :placeholder="t('custom_value')" data-testid="sampling-custom-value" />
					<ShadButton variant="ghost" size="icon" class="h-8 w-8 flex-shrink-0" :aria-label="t('remove_custom')" @click="customFields.splice(idx, 1)">
						<X class="h-3.5 w-3.5" />
					</ShadButton>
				</div>
				<ShadButton variant="outline" size="sm" class="h-8 w-full text-xs" data-testid="sampling-add-custom" @click="customFields.push({key: '', value: ''})">
					<Plus class="h-3.5 w-3.5" />
					{{ t('add_custom') }}
				</ShadButton>
			</div>

			<div class="flex justify-between gap-2 pt-1">
				<ShadButton variant="ghost" size="sm" data-testid="sampling-reset" :disabled="saving" @click="reset">
					{{ presetDefaults ? store.getTranslation('presets.sampling.reset') : t('reset') }}
				</ShadButton>
				<ShadButton size="sm" data-testid="sampling-save" :disabled="saving" @click="save">
					<Loader2 v-if="saving" class="h-3.5 w-3.5 animate-spin" />
					{{ t('save') }}
				</ShadButton>
			</div>
		</ShadPopoverContent>
	</ShadPopover>
</template>

<script setup lang="ts">
import {Loader2, Plus, SlidersHorizontal, X} from 'lucide-vue-next';
import {cn} from '~/lib/utils';
import {useMainStore} from '~/stores';
import {useChatStore} from '~/stores/chatStore';
import {useModelStore} from '~/stores/modelStore';
import type {ChatSampling} from '~/types/chat';
import ParamCard from '~/components/settings/models/ParamCard.vue';

type NumericKey = 'temperature' | 'top_p' | 'max_tokens' | 'frequency_penalty' | 'presence_penalty' | 'seed';
type SliderParam = {key: Exclude<NumericKey, 'seed'>; def: number; min: number; max: number; step: number; show: (v: number) => string; labels: string[]};

const store = useMainStore();
const chatStore = useChatStore();
const modelStore = useModelStore();

const preset = computed(() => modelStore.selectedModel?.preset ?? null);
const presetName = computed(() => preset.value?.name ?? '');

function presetValue(key: NumericKey): number | null {
	const value = preset.value?.sampling[key];
	return typeof value === 'number' ? value : null;
}

const presetDefaults = computed(() => {
	const parts = numericKeys.filter(key => presetValue(key) !== null).map(key => `${editor(key)} ${presetValue(key)}`);
	return parts.length > 0 ? parts.join(', ') : null;
});

const t = (key: string) => store.getTranslation(`chat.sampling.${key}`);
const editor = (key: string) => store.getTranslation(`settings.models.editor.${key}`);

const fixed = (v: number) => v.toFixed(2);
const signed = (v: number) => (v >= 0 ? '+' : '') + v.toFixed(2);
const penalty = {min: -2, max: 2, step: 0.01, show: signed, labels: ['-2', '0', '+2']};

const sliders: SliderParam[] = [
	{key: 'temperature', def: 0.7, min: 0, max: 2, step: 0.01, show: fixed, labels: ['0', '2']},
	{key: 'top_p', def: 1, min: 0, max: 1, step: 0.01, show: fixed, labels: ['0', '1']},
	{key: 'max_tokens', def: 4096, min: 256, max: 128000, step: 256, show: v => v.toLocaleString(), labels: ['256', '128 000']},
	{key: 'frequency_penalty', def: 0, ...penalty},
	{key: 'presence_penalty', def: 0, ...penalty},
];

const numericKeys: NumericKey[] = ['temperature', 'top_p', 'max_tokens', 'frequency_penalty', 'presence_penalty', 'seed'];

const open = ref(false);
const saving = ref(false);
const form = reactive<Record<NumericKey, number | null>>({temperature: null, top_p: null, max_tokens: null, frequency_penalty: null, presence_penalty: null, seed: null});
const customFields = ref<{key: string; value: string}[]>([]);

const current = computed<ChatSampling>(() => chatStore.currentSampling);
const activeCount = computed(() => chatStore.samplingActiveCount);

function load() {
	for (const key of numericKeys) form[key] = current.value[key] ?? null;
	customFields.value = Object.entries(current.value.custom ?? {}).map(([key, value]) => ({key, value: JSON.stringify(value)}));
}

watch(open, isOpen => {
	if (isOpen) load();
});

function toggle(key: NumericKey, def: number) {
	form[key] = form[key] !== null ? null : def;
}

function parseValue(raw: string): unknown {
	try {
		return JSON.parse(raw);
	} catch {
		return raw;
	}
}

function buildSampling(): ChatSampling {
	const sampling: ChatSampling = {};
	for (const key of numericKeys) {
		const value = form[key];
		if (typeof value === 'number' && Number.isFinite(value)) sampling[key] = value;
	}
	const custom: Record<string, unknown> = {};
	for (const field of customFields.value) {
		const key = field.key.trim();
		if (key) custom[key] = parseValue(field.value.trim());
	}
	if (Object.keys(custom).length > 0) sampling.custom = custom;
	return sampling;
}

async function persist(sampling: ChatSampling) {
	saving.value = true;
	try {
		await chatStore.saveSampling(sampling);
		open.value = false;
		store.toast(t('saved'), {type: 'success'});
	} catch (e: any) {
		store.toast(e?.data?.errors?.[0]?.message || t('save_failed'), {type: 'error'});
	} finally {
		saving.value = false;
	}
}

function save() {
	return persist(buildSampling());
}

function reset() {
	return persist({});
}
</script>
