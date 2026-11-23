<template>
	<ShadPopover v-model:open="open">
		<ShadPopoverTrigger as-child>
			<ShadButton
				variant="ghost"
				data-testid="composer-settings"
				:aria-label="triggerLabel"
				:class="cn('h-8 w-auto shrink-0 gap-1.5 px-2', chips.length > 0 ? 'text-primary' : 'text-muted-foreground')"
			>
				<Settings2 class="h-4 w-4" />
				<span
					v-for="chip in chips"
					:key="chip.key"
					:data-testid="`composer-chip-${chip.key}`"
					class="hidden max-w-28 truncate rounded bg-primary/10 px-1.5 py-0.5 text-[11px] font-medium leading-none sm:inline-block"
				>
					{{ chip.label }}
				</span>
				<span v-if="chips.length > 0" class="text-xs font-medium tabular-nums sm:hidden" data-testid="composer-settings-count">{{ chips.length }}</span>
			</ShadButton>
		</ShadPopoverTrigger>
		<ShadPopoverContent align="start" class="w-72 max-w-[calc(100vw-1rem)] space-y-1 p-2" data-testid="composer-settings-panel">
			<p class="px-1 pb-1 text-sm font-semibold text-foreground">{{ store.getTranslation('chat.composer.settings') }}</p>
			<div v-if="showReasoning" role="group" :aria-label="store.getTranslation('chat.composer.reasoning')" class="flex items-center justify-between gap-3 px-1">
				<span class="text-xs text-muted-foreground">{{ store.getTranslation('chat.composer.reasoning') }}</span>
				<ReasoningSelector class="h-8 border-none! bg-transparent! shadow-none! hover:bg-muted/50" />
			</div>
			<div role="group" :aria-label="store.getTranslation('chat.sampling.title')" class="flex items-center justify-between gap-3 px-1">
				<span class="text-xs text-muted-foreground">{{ store.getTranslation('chat.sampling.title') }}</span>
				<SamplingSelector />
			</div>
			<div v-if="modelStore.providerSelectorVisible" role="group" :aria-label="store.getTranslation('chat.provider_selector.title')" class="flex items-center justify-between gap-3 px-1">
				<span class="text-xs text-muted-foreground">{{ store.getTranslation('chat.provider_selector.title') }}</span>
				<ProviderSelector class="h-8" />
			</div>
			<div v-if="knowledgeVisible" role="group" :aria-label="store.getTranslation('chat.composer.knowledge')" class="space-y-0.5 px-1 pt-1" data-testid="composer-knowledge">
				<span class="text-xs text-muted-foreground">{{ store.getTranslation('chat.composer.knowledge') }}</span>
				<label
					v-for="base in knowledgeStore.usable"
					:key="base.id"
					class="flex items-center gap-2 rounded-md px-1 py-1 text-sm text-foreground"
					:class="knowledgeAllowed(base) ? 'cursor-pointer hover:bg-muted/50' : 'cursor-not-allowed opacity-60'"
					:title="knowledgeAllowed(base) ? (base.description ?? '') : store.getTranslation('chat.composer.knowledge_not_allowed')"
					:data-testid="`composer-knowledge-${base.name}`"
					:data-disabled="!knowledgeAllowed(base)"
				>
					<ShadCheckbox :model-value="knowledgeStore.selected.includes(base.id)" :disabled="!knowledgeAllowed(base)" @update:model-value="knowledgeStore.toggle(base.id)" />
					<span class="min-w-0 flex-1 truncate">{{ base.name }}</span>
					<span v-if="!knowledgeAllowed(base)" class="shrink-0 text-xs text-muted-foreground">{{ store.getTranslation('chat.composer.knowledge_not_allowed') }}</span>
				</label>
			</div>
		</ShadPopoverContent>
	</ShadPopover>
</template>

<script setup lang="ts">
import {Settings2} from 'lucide-vue-next';
import {cn} from '~/lib/utils';
import {useMainStore} from '~/stores';
import {useChatStore} from '~/stores/chatStore';
import {useModelStore} from '~/stores/modelStore';
import {useKnowledgeStore} from '~/stores/knowledgeStore';
import type {KnowledgeBaseResponse} from '~/types/generated/KnowledgeBaseResponse';
import ReasoningSelector from './ReasoningSelector.vue';
import SamplingSelector from './SamplingSelector.vue';
import ProviderSelector from './ProviderSelector.vue';

const store = useMainStore();
const chatStore = useChatStore();
const modelStore = useModelStore();
const knowledgeStore = useKnowledgeStore();
const open = ref(false);
const knowledgeEnabled = computed(() => Boolean(store.base?.rag?.enabled));
/** Whether the knowledge base's model access lets it reach the selected model; the server checks this again. */
function knowledgeAllowed(base: KnowledgeBaseResponse) {
	const model = modelStore.selectedModel;
	const modelId = model?.level?.model_id ?? model?.id;
	return !base.allowed_model_ids || !modelId || base.allowed_model_ids.includes(modelId);
}
const knowledgeVisible = computed(() => knowledgeEnabled.value && knowledgeStore.usable.length > 0);

const effortKeys: Record<string, string> = {minimal: 'minimal', low: 'low', medium: 'medium', high: 'high', xhigh: 'extra_high'};

const showReasoning = computed(() => !modelStore.selectedModel?.level && modelStore.hasConfigurableReasoningCapability(modelStore.selectedModel));

const reasoningLabel = computed(() => {
	if (!showReasoning.value) return null;
	const parts: string[] = [];
	const effort = modelStore.reasoningEffort;
	if (effort && effort !== 'none') parts.push(effortKeys[effort] ? store.getTranslation(`chat.reasoning_selector.${effortKeys[effort]}`) : effort);
	if (modelStore.reasoningBudget) parts.push(`${Math.round(modelStore.reasoningBudget / 1024)}K`);
	return parts.length > 0 ? parts.join(' / ') : null;
});

const chips = computed(() => {
	const list: {key: string; label: string}[] = [];
	if (reasoningLabel.value) list.push({key: 'reasoning', label: reasoningLabel.value});
	if (chatStore.samplingActiveCount > 0) {
		list.push({key: 'sampling', label: store.getTranslation('chat.composer.sampling_chip', {count: chatStore.samplingActiveCount})});
	}
	if (modelStore.providerSelectorVisible && modelStore.selectedProviderName) list.push({key: 'provider', label: modelStore.selectedProviderName});
	const attached = knowledgeStore.usable.filter(base => knowledgeStore.selected.includes(base.id)).length;
	if (knowledgeEnabled.value && attached > 0) list.push({key: 'knowledge', label: store.getTranslation('chat.composer.knowledge_chip', {count: attached})});
	return list;
});

const triggerLabel = computed(() => {
	const title = store.getTranslation('chat.composer.settings');
	return chips.value.length > 0 ? `${title}: ${chips.value.map(chip => chip.label).join(', ')}` : title;
});

watch(
	knowledgeEnabled,
	enabled => {
		if (enabled) knowledgeStore.ensure();
	},
	{immediate: true},
);

watch(
	() => modelStore.providerSelectorVisible,
	visible => {
		if (visible) modelStore.loadProviderOptions();
	},
	{immediate: true},
);
</script>
