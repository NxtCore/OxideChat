<template>
	<ShadSelect :model-value="chatStore.selectedModel?.model_id || ''" @update:model-value="handleModelChange">
		<ShadSelectTrigger :class="cn('w-auto', props.class)">
			<ShadSelectValue>
				<div class="flex items-center gap-2">
					<div
						v-if="iconStore.getProviderIcon(chatStore.selectedModel?.provider_name)?.type === 'svg'"
						class="h-4 w-4 flex items-center justify-center [&>svg]:h-full [&>svg]:w-full [&>svg]:display-block"
						v-html="iconStore.getProviderIcon(chatStore.selectedModel?.provider_name)?.icon"
					/>
					<div
						v-else-if="iconStore.getProviderIcon(chatStore.selectedModel?.provider_name)?.type === 'png'"
						class="h-4 w-4 flex items-center justify-center [&>svg]:h-full [&>svg]:w-full [&>svg]:display-block"
					>
						<img :src="iconStore.getProviderIcon(chatStore.selectedModel?.provider_name)?.icon" alt="Provider icon" />
					</div>
					<span class="max-w-[150px] truncate text-xs font-medium">
						{{ chatStore.selectedModel?.display_name || 'Select model' }}
					</span>
				</div>
			</ShadSelectValue>
		</ShadSelectTrigger>
		<ShadSelectContent class="max-h-[400px]">
			<div class="p-2 sticky -top-2 z-10 bg-popover border-b">
				<ShadInput
					v-model="search"
					type="text"
					placeholder="Search models..."
					class="w-full rounded-md border border-border bg-background px-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-primary"
				/>
			</div>
			<template v-if="filteredFavorites.length > 0">
				<ShadSelectLabel class="flex items-center gap-2 px-2 py-1.5 text-sm font-semibold">
					<Star class="h-3 w-3 text-primary" />
					Favorites
				</ShadSelectLabel>
				<ShadSelectGroup>
					<ShadSelectItem v-for="model in filteredFavorites" :key="model.id" :value="model.model_id">
						<div class="flex items-center gap-2 flex-1 min-w-0">
							<div
								v-if="iconStore.getProviderIcon(model.provider_name)?.type === 'svg'"
								class="h-4 w-4 flex items-center justify-center [&>svg]:h-full [&>svg]:w-full [&>svg]:display-block"
								v-html="iconStore.getProviderIcon(model.provider_name)?.icon"
							/>
							<div
								v-else-if="iconStore.getProviderIcon(model.provider_name)?.type === 'png'"
								class="h-4 w-4 flex items-center justify-center [&>svg]:h-full [&>svg]:w-full [&>svg]:display-block"
							>
								<img :src="iconStore.getProviderIcon(model.provider_name)?.icon" alt="Provider icon" />
							</div>
							<span class="truncate">{{ model.display_name }}</span>
						</div>
					</ShadSelectItem>
				</ShadSelectGroup>
				<ShadSelectSeparator />
			</template>
			<template v-for="(models, provider) in filteredGrouped" :key="provider">
				<ShadSelectLabel class="px-2 py-1.5 text-sm font-semibold">
					{{ provider }}
				</ShadSelectLabel>
				<ShadSelectGroup>
					<ShadSelectItem v-for="model in models" :key="model.id" :value="model.model_id">
						<div class="flex items-center gap-2 flex-1 min-w-0">
							<div
								v-if="iconStore.getProviderIcon(model.provider_name)?.type === 'svg'"
								class="h-4 w-4 flex items-center justify-center [&>svg]:h-full [&>svg]:w-full [&>svg]:display-block"
								v-html="iconStore.getProviderIcon(model.provider_name)?.icon"
							/>
							<div
								v-else-if="iconStore.getProviderIcon(model.provider_name)?.type === 'png'"
								class="h-4 w-4 flex items-center justify-center [&>svg]:h-full [&>svg]:w-full [&>svg]:display-block"
							>
								<img :src="iconStore.getProviderIcon(model.provider_name)?.icon" alt="Provider icon" />
							</div>
							<span class="truncate">{{ model.display_name }}</span>
						</div>
					</ShadSelectItem>
				</ShadSelectGroup>
			</template>
			<div v-if="Object.keys(filteredGrouped).length === 0 && filteredFavorites.length === 0" class="p-4 text-center text-sm text-muted-foreground">
				No models found
			</div>
		</ShadSelectContent>
	</ShadSelect>
</template>

<script setup lang="ts">
import {Bot, Star} from 'lucide-vue-next';
import type {Model} from '~/types/chat';
import {useChatStore} from '~/stores/chatStore';
import {cn} from '~/lib/utils';

const props = defineProps<{
	class?: string;
}>();

const chatStore = useChatStore();
const iconStore = useIconsStore();
const search = ref('');

const filteredFavorites = computed(() => {
	const query = search.value.toLowerCase();
	return chatStore.favoriteModels.filter(m => m.display_name.toLowerCase().includes(query) || m.model_id.toLowerCase().includes(query));
});

const filteredGrouped = computed(() => {
	const query = search.value.toLowerCase();
	const result: Record<string, Model[]> = {};

	for (const [provider, models] of Object.entries(chatStore.groupedModels)) {
		const filtered = models.filter(m => m.display_name.toLowerCase().includes(query) || m.model_id.toLowerCase().includes(query));
		if (filtered.length > 0) {
			result[provider] = filtered;
		}
	}

	return result;
});

function handleModelChange(value: string | number | bigint | boolean | Record<string, any> | null) {
	if (typeof value !== 'string') return;
	const model = chatStore.models.find(m => m.model_id === value);
	if (model) {
		chatStore.setSelectedModel(model);
	}
}

watch(
	() => chatStore.activeChat?.id,
	(newVal, oldVal) => {
		console.log('Active chat changed:', newVal, oldVal);
		console.log('Messages:', chatStore.messages);
		if (oldVal !== newVal && newVal && chatStore.messages.length > 0) {
			const model = chatStore.models.find(m => m.id === chatStore.messages[chatStore.messages.length - 1]?.model_id);
			console.log('Selected model:', model);
			if (model) {
				search.value = '';
				chatStore.setSelectedModel(model);
			}
		}
	}
);
</script>
