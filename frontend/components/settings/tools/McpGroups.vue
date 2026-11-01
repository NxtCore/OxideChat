<template>
	<div class="space-y-3 mb-4">
		<h3 class="text-sm font-medium text-muted-foreground px-1">{{ store.getTranslation('mcp.manage_title') }}</h3>
		<div v-for="group in groups" :key="group.server_id" class="rounded-lg border border-border bg-card">
			<div class="flex items-center gap-2 p-4">
				<button type="button" class="flex items-center gap-4 min-w-0 flex-1 text-left" @click="toggleExpanded(group.server_id)">
					<div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-green-500/10">
						<Server class="h-5 w-5 text-green-600" />
					</div>
					<div class="min-w-0">
						<div class="flex items-center gap-2">
							<h3 class="font-medium text-foreground">{{ group.server_name }}</h3>
							<span class="inline-flex items-center rounded-full bg-green-500/10 px-2 py-0.5 text-xs font-medium text-green-500">MCP</span>
						</div>
						<p class="text-sm text-muted-foreground">{{ group.tools.length }} {{ store.getTranslation('mcp.discovered_tools') }}</p>
					</div>
				</button>
				<div class="shrink-0 flex items-center gap-2">
					<ShadButton variant="outline" size="sm" @click.stop="emit('edit', group.server_id)">
						<Pencil class="h-4 w-4" />
					</ShadButton>
					<ChevronDown
						class="h-4 w-4 text-muted-foreground transition-transform cursor-pointer"
						:class="expanded[group.server_id] ? 'rotate-180' : ''"
						@click="toggleExpanded(group.server_id)"
					/>
				</div>
			</div>
			<div v-if="expanded[group.server_id]" class="border-t border-border">
				<div v-for="tool in group.tools" :key="tool.id" class="flex items-center justify-between gap-4 px-4 py-3 border-b border-border/50 last:border-0">
					<div class="min-w-0">
						<p class="text-sm font-medium text-foreground">{{ tool.display_name || tool.name }}</p>
						<p v-if="tool.description" class="text-xs text-muted-foreground truncate">{{ tool.description }}</p>
					</div>
					<Switch :modelValue="tool.is_enabled" @update:modelValue="(val: boolean) => emit('toggle', tool, val)" />
				</div>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import {Server, ChevronDown, Pencil} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {Switch} from '@/components/ui/switch';
import type {Tool} from '~/types/tools';

defineProps<{
	groups: {server_id: string; server_name: string; tools: Tool[]}[];
}>();

const emit = defineEmits<{
	edit: [serverId: string];
	toggle: [tool: Tool, enabled: boolean];
}>();

const store = useMainStore();
const expanded = ref<Record<string, boolean>>({});

function toggleExpanded(serverId: string) {
	expanded.value[serverId] = !expanded.value[serverId];
}
</script>
