<template>
	<div v-if="!isTokenBudget" class="space-y-2">
		<ShadSelect v-model="effortLevel" @update:modelValue="handleEffortChange" :disabled="chatStore.isReasoningRequired">
			<ShadSelectTrigger
				:class="
					cn(
						'w-auto',
						props.class,
						chatStore.reasoningEffort && chatStore.reasoningEffort !== 'none' ? 'text-primary border-primary/50' : 'text-muted-foreground'
					)
				"
			>
				<Brain class="h-4 w-4" />
				<span v-if="chatStore.reasoningEffort && chatStore.reasoningEffort !== 'none'" class="text-xs font-medium">
					{{ displayLabel }}
				</span>
			</ShadSelectTrigger>
			<ShadSelectContent>
				<ShadSelectGroup>
					<ShadSelectItem v-for="(label, index) in effortLabels" :key="availableEffortLevels[index]" :value="availableEffortLevels[index]">
						{{ label }}
					</ShadSelectItem>
				</ShadSelectGroup>
			</ShadSelectContent>
		</ShadSelect>
	</div>
	<div v-else class="space-y-2">
		<Popover>
			<PopoverTrigger as-child>
				<ShadButton
					variant="outline"
					:class="cn('w-auto justify-between font-normal', props.class, tokenBudget ? 'text-primary border-primary/50' : 'text-muted-foreground')"
				>
					<div class="flex items-center gap-2">
						<Brain class="h-4 w-4" />
						<span v-if="tokenBudget" class="text-xs font-medium">{{ displayLabel }}</span>
						<span v-else class="text-xs">{{ store.getTranslation('chat.reasoning_selector.disabled') }}</span>
					</div>
				</ShadButton>
			</PopoverTrigger>
			<PopoverContent class="w-80">
				<div class="space-y-4">
					<div class="flex justify-between items-center">
						<h4 class="font-medium leading-none">{{ store.getTranslation('chat.reasoning_selector.token_limit') }}</h4>
						<span class="text-xs text-muted-foreground">{{ minTokens }}-{{ maxTokens }}</span>
					</div>
					<div class="pt-2">
						<Slider v-model="sliderValue" :min="minTokens" :max="maxTokens" :step="1024" @update:modelValue="handleSliderChange" />
					</div>
					<div class="flex items-center gap-2">
						<ShadInput
							v-model.number="tempTokenInput"
							type="number"
							:min="minTokens"
							:max="maxTokens"
							:placeholder="store.getTranslation('chat.reasoning_selector.disabled')"
							class="h-8"
							@input="handleInputChange"
						/>
						<ShadButton variant="ghost" size="sm" class="h-8 px-2 text-muted-foreground hover:text-foreground" @click="resetToDisabled">{{
							store.getTranslation('chat.reasoning_selector.disabled')
						}}</ShadButton>
					</div>
				</div>
			</PopoverContent>
		</Popover>
	</div>
</template>

<script setup lang="ts">
import {Brain} from 'lucide-vue-next';
import {useChatStore} from '~/stores/chatStore';
import {useMainStore} from '~/stores';
import {cn} from '~/lib/utils';
import {Popover, PopoverTrigger, PopoverContent} from '~/components/ui/popover';
import {Slider} from '~/components/ui/slider';

const store = useMainStore();

const props = defineProps<{
	class?: string;
}>();

const chatStore = useChatStore();

const effortLevel = ref('');
const tokenBudget = ref<number | undefined>(undefined);
const tempTokenInput = ref<number | undefined>(undefined);

const sliderValue = computed({
	get: () => [tokenBudget.value || minTokens.value],
	set: val => {
		// Handled in handleSliderChange
	},
});

const isTokenBudget = computed(() => {
	const capabilities = chatStore.selectedModel?.capabilities || [];
	return capabilities.some(cap => cap.startsWith('REASONING_BUDGET_TOKENS_'));
});

const availableEffortLevels = computed(() => {
	const capabilities = chatStore.selectedModel?.capabilities || [];
	const efforts = ['NONE', 'MINIMAL', 'LOW', 'MEDIUM', 'HIGH', 'XHIGH'];
	return efforts.filter(effort => capabilities.includes(`REASONING_EFFORT_${effort}`));
});

const effortLabels = computed(() => {
	return availableEffortLevels.value.map(effort => {
		switch (effort) {
			case 'NONE':
				return store.getTranslation('chat.reasoning_selector.none');
			case 'MINIMAL':
				return store.getTranslation('chat.reasoning_selector.minimal');
			case 'LOW':
				return store.getTranslation('chat.reasoning_selector.low');
			case 'MEDIUM':
				return store.getTranslation('chat.reasoning_selector.medium');
			case 'HIGH':
				return store.getTranslation('chat.reasoning_selector.high');
			case 'XHIGH':
				return store.getTranslation('chat.reasoning_selector.extra_high');
			default:
				return effort;
		}
	});
});

const tokenBudgetRange = computed(() => {
	const capabilities = chatStore.selectedModel?.capabilities || [];

	for (const cap of capabilities) {
		const match = cap.match(/REASONING_BUDGET_TOKENS_(\d+)_(\d+)/);
		if (match) {
			return {
				min: parseInt(match[1] || '0'),
				max: parseInt(match[2] || '0'),
			};
		}
	}

	return {min: 1024, max: 32768};
});

const minTokens = computed(() => tokenBudgetRange.value.min);
const maxTokens = computed(() => tokenBudgetRange.value.max);

const displayLabel = computed(() => {
	if (isTokenBudget.value && tokenBudget.value) {
		return `${Math.round(tokenBudget.value / 1024)}K ${store.getTranslation('chat.reasoning_selector.tokens')}`;
	}

	if (effortLevel.value) {
		const index = availableEffortLevels.value.indexOf(effortLevel.value);
		return effortLabels.value[index] || store.getTranslation('chat.reasoning_selector.medium');
	}

	return store.getTranslation('chat.reasoning_selector.disabled');
});

onMounted(() => {
	syncFromStore();
});

function syncFromStore() {
	const currentEffort = chatStore.reasoningEffort;
	if (currentEffort) {
		if (isTokenBudget.value) {
			const num = parseInt(currentEffort);
			if (!isNaN(num)) {
				tokenBudget.value = num;
				tempTokenInput.value = num;
			}
		} else {
			const upperEffort = currentEffort.toUpperCase().replace(/-/g, '_');
			if (availableEffortLevels.value.includes(upperEffort)) {
				effortLevel.value = upperEffort;
			}
		}
	} else {
		tokenBudget.value = undefined;
		tempTokenInput.value = undefined;
		effortLevel.value = '';
	}
}

// Keep UI in sync when the store changes
watch(
	() => chatStore.reasoningEffort,
	() => {
		syncFromStore();
	}
);

// When available effort levels change (model switch), ensure the selected UI value is valid
watch(availableEffortLevels, newV => {
	if (effortLevel.value && !newV.includes(effortLevel.value)) {
		effortLevel.value = '';
	}
});

function handleEffortChange(value: any) {
	const effortStr = value ? String(value) : '';
	const effort = effortStr ? effortStr.toLowerCase().replace(/_/g, '-') : null;
	chatStore.setReasoningEffort(effort);
}

function handleSliderChange(val: number[] | undefined) {
	if (val && val.length > 0) {
		const newValue = val[0];
		if (newValue !== undefined) {
			tokenBudget.value = newValue;
			tempTokenInput.value = newValue;
			chatStore.setReasoningEffort(newValue.toString(), true);
		}
	}
}

function handleInputChange() {
	if (tempTokenInput.value !== undefined) {
		let val = tempTokenInput.value;
		if (val < minTokens.value) val = minTokens.value;
		if (val > maxTokens.value) val = maxTokens.value;

		tokenBudget.value = val;
		chatStore.setReasoningEffort(val.toString());
	} else {
		resetToDisabled();
	}
}

function resetToDisabled() {
	tokenBudget.value = undefined;
	tempTokenInput.value = undefined;
	chatStore.setReasoningEffort(null, true);
}
</script>
