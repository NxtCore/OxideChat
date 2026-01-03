<template>
	<ShadSelect :model-value="enabledToolsValues" multiple @update:model-value="handleToolsChange">
		<ShadSelectTrigger class="w-auto" :class="chatStore.enabledTools.length > 0 ? 'text-primary border-primary/50' : 'text-muted-foreground'">
			<ShadSelectValue>
				<div class="flex items-center gap-2">
					<Wrench class="h-4 w-4" />
					<span v-if="chatStore.enabledTools.length > 0" class="text-xs">
						{{ chatStore.enabledTools.length }}
					</span>
				</div>
			</ShadSelectValue>
		</ShadSelectTrigger>
		<ShadSelectContent class="max-h-[400px] w-80">
			<div class="flex flex-col gap-2 p-2 border-b border-border">
				<ShadInput
					v-model="search"
					type="text"
					placeholder="Search tools..."
					class="w-full rounded-md border border-border bg-background px-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-primary"
				/>
				<p v-if="!hasToolSupport" class="text-xs text-amber-500">Selected model doesn't support tools</p>
			</div>
			<ShadSelectGroup>
				<ShadSelectItem v-for="tool in filteredTools" :key="tool.id" :value="tool.id" @click.prevent="toggleTool(tool.id)">
					<div class="flex items-center justify-between gap-2">
						<component :is="tool.icon" class="h-4 w-4 shrink-0 text-muted-foreground" />
						<ShadLabel class="font-normal flex flex-col items-start gap-0">
							<span class="text-sm font-medium text-foreground">{{ tool.name }}</span>
							<span class="text-xs text-muted-foreground truncate">{{ tool.description }}</span>
						</ShadLabel>
					</div>
				</ShadSelectItem>
			</ShadSelectGroup>
			<div v-if="filteredTools.length === 0" class="p-4 text-center text-sm text-muted-foreground">No tools found</div>
		</ShadSelectContent>
	</ShadSelect>
</template>

<script setup lang="ts">
import {Check, Wrench, Globe, Code, Calculator} from 'lucide-vue-next';
import {useChatStore} from '~/stores/chatStore';

interface Tool {
	id: string;
	name: string;
	description: string;
	icon: any;
}

const chatStore = useChatStore();
const search = ref('');

const availableTools = ref<Tool[]>([
	{
		id: 'web_search',
		name: 'Web Search',
		description: 'Search the web for current information',
		icon: Globe,
	},
	{
		id: 'code_execution',
		name: 'Code Execution',
		description: 'Run code in a sandboxed environment',
		icon: Code,
	},
	{
		id: 'calculator',
		name: 'Calculator',
		description: 'Perform precise calculations',
		icon: Calculator,
	},
]);

const enabledToolsValues = ref<string[]>([]);

const hasToolSupport = computed(() => {
	return chatStore.selectedModel?.capabilities.includes('TOOLS');
});

const filteredTools = computed(() => {
	if (!search.value) return availableTools.value;
	const query = search.value.toLowerCase();
	return availableTools.value.filter(tool => tool.name.toLowerCase().includes(query) || tool.description.toLowerCase().includes(query));
});

function toggleTool(toolId: string) {
	chatStore.toggleTool(toolId);
}

function handleToolsChange(
	value: string | number | bigint | boolean | Record<string, any> | null | Array<string | number | bigint | boolean | Record<string, any> | null>
) {
	if (Array.isArray(value)) {
		const newEnabledTools = value.filter((v): v is string => typeof v === 'string');
		enabledToolsValues.value = newEnabledTools;
	}
}

watch(
	() => chatStore.enabledTools,
	newTools => {
		enabledToolsValues.value = newTools;
	},
	{immediate: true}
);

onMounted(async () => {
	try {
		const {$customFetch} = useNuxtApp();
		const fetchedTools = await $customFetch('/api/v1/tools');
		if (Array.isArray(fetchedTools)) {
			availableTools.value = fetchedTools.map((tool: any) => ({
				id: tool.id,
				name: tool.name,
				description: tool.description,
				icon: Globe,
			}));
		}
	} catch (error) {
		console.log('Using default tools (API not available):', error);
	}
});
</script>
