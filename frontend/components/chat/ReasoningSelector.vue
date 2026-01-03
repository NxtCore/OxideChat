<template>
	<div v-if="!isTokenBudget" class="space-y-2">
		<ShadSelect v-model="effortLevel" @update:modelValue="handleEffortChange">
			<ShadSelectTrigger :class="chatStore.reasoningEffort && chatStore.reasoningEffort !== 'none' ? 'text-primary border-primary/50' : 'text-muted-foreground'">
				<Brain class="h-4 w-4" />
				<ShadSelectValue v-if="chatStore.reasoningEffort && chatStore.reasoningEffort !== 'none'" />
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
		<ShadLabel class="text-sm text-muted-foreground"> Token Budget ({{ minTokens }}-{{ maxTokens }}) </ShadLabel>
		<ShadInput
			v-model.number="tokenBudget"
			type="number"
			:min="minTokens"
			:max="maxTokens"
			:step="1024"
			placeholder="Auto"
			class="w-full rounded-md border border-border bg-background px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-primary"
			@input="handleTokenChange"
		/>
		<p class="text-xs text-muted-foreground">Leave empty for automatic selection</p>
	</div>
</template>

<script setup lang="ts">
import {Brain} from 'lucide-vue-next';
import {useChatStore} from '~/stores/chatStore';

const chatStore = useChatStore();

const effortLevel = ref('');
const tokenBudget = ref<number | null>(null);

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
				return 'None';
			case 'MINIMAL':
				return 'Minimal';
			case 'LOW':
				return 'Low';
			case 'MEDIUM':
				return 'Medium';
			case 'HIGH':
				return 'High';
			case 'XHIGH':
				return 'Extra High';
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
				min: parseInt(match[1]),
				max: parseInt(match[2]),
			};
		}
	}

	return {min: 1024, max: 32768};
});

const minTokens = computed(() => tokenBudgetRange.value.min);
const maxTokens = computed(() => tokenBudgetRange.value.max);

const displayLabel = computed(() => {
	if (isTokenBudget.value && tokenBudget.value) {
		return `${tokenBudget.value / 1024}K`;
	}

	if (effortLevel.value) {
		const index = availableEffortLevels.value.indexOf(effortLevel.value);
		return effortLabels.value[index] || 'Medium';
	}

	return 'Auto';
});

onMounted(() => {
	const currentEffort = chatStore.reasoningEffort;
	if (currentEffort) {
		if (isTokenBudget.value) {
			const num = parseInt(currentEffort);
			if (!isNaN(num)) {
				tokenBudget.value = num;
			}
		} else {
			const upperEffort = currentEffort.toUpperCase().replace(/-/g, '_');
			if (availableEffortLevels.value.includes(upperEffort)) {
				effortLevel.value = upperEffort;
			}
		}
	}
});

function handleEffortChange(value: string) {
	const effort = value ? value.toLowerCase().replace(/_/g, '-') : null;
	chatStore.setReasoningEffort(effort);
}

function handleTokenChange() {
	if (tokenBudget.value && tokenBudget.value >= minTokens.value && tokenBudget.value <= maxTokens.value) {
		chatStore.setReasoningEffort(tokenBudget.value.toString());
	} else if (!tokenBudget.value) {
		chatStore.setReasoningEffort(null);
	}
}

function clearReasoning() {
	chatStore.setReasoningEffort(null);
	effortLevel.value = '';
	tokenBudget.value = null;
}
</script>
