<template>
	<ShadPopover v-model:open="open">
		<ShadPopoverTrigger as-child>
			<ShadButton
				type="button"
				:class="
					cn(
						'inline-flex h-8 items-center gap-1.5 rounded-md px-2 text-xs font-medium transition-colors hover:bg-muted',
						chatStore.enabledTools.length > 0 ? 'text-primary' : 'text-muted-foreground',
						props.class
					)
				"
			>
				<Wrench class="h-4 w-4" />
				<span v-if="chatStore.enabledTools.length > 0">{{ chatStore.enabledTools.length }}</span>
			</ShadButton>
		</ShadPopoverTrigger>
		<ShadPopoverContent align="start" class="w-80 p-0">
			<div class="flex flex-col">
				<div class="border-b border-border p-2">
					<div class="flex items-center justify-between px-1 pb-2">
						<span class="text-sm font-medium text-foreground">{{ store.getTranslation('chat.tool_selector.title') }}</span>
						<span v-if="chatStore.enabledTools.length > 0" class="text-xs text-muted-foreground">
							{{ store.getTranslation('chat.tool_selector.count', {count: chatStore.enabledTools.length}) }}
						</span>
					</div>
					<div class="relative">
						<Search class="pointer-events-none absolute top-1/2 left-2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
						<ShadInput
							v-model="search"
							type="text"
							:placeholder="store.getTranslation('chat.tool_selector.search_tools')"
							class="h-8 rounded-md border-border bg-background pl-7 pr-2 text-xs"
						/>
					</div>
					<p v-if="!hasToolSupport" class="px-1 pt-2 text-xs text-amber-500">
						{{ store.getTranslation('chat.tool_selector.no_tool_support') }}
					</p>
				</div>

				<div class="max-h-72 overflow-y-auto p-1">
					<template v-if="groupedHeads.length > 0">
						<template v-for="group in groupedHeads" :key="group.kind">
							<div class="flex items-center gap-1.5 px-2 py-1.5">
								<component :is="group.icon" class="h-3 w-3 text-muted-foreground" />
								<span class="text-[10px] font-semibold tracking-wide text-muted-foreground uppercase">{{ group.label }}</span>
								<span class="ml-auto text-[10px] text-muted-foreground">{{ group.heads.length }}</span>
							</div>

							<template v-for="head in group.heads" :key="head.id">
								<ShadButton
									v-if="!head.collapsible"
									type="button"
									class="group flex w-full items-start gap-2 rounded-md px-2 py-1.5 text-left transition-colors hover:bg-muted"
									@click="toggleTool(head.items[0].id)"
								>
									<div
										class="mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center rounded border transition-colors"
										:class="isSelected(head.items[0].id) ? 'border-primary bg-primary text-primary-foreground' : 'border-input bg-background'"
									>
										<Check v-if="isSelected(head.items[0].id)" class="h-3 w-3" />
									</div>
									<div class="min-w-0 flex-1">
										<div class="flex items-center justify-between gap-2">
											<span class="truncate text-xs font-medium text-foreground">{{ head.label }}</span>
											<span v-if="head.itemCount > 1" class="shrink-0 text-[10px] text-muted-foreground">{{ head.itemCount }}</span>
										</div>
										<p v-if="head.description" class="truncate text-[11px] text-muted-foreground">{{ head.description }}</p>
									</div>
								</ShadButton>

								<ShadCollapsible v-else :open="isExpanded(head.id)" @update:open="val => setExpanded(head.id, val)" as-child>
									<div class="rounded-md transition-colors hover:bg-muted/50">
										<div class="flex items-center gap-1.5 px-2 py-1.5">
											<ShadButton type="button" class="shrink-0" :aria-label="store.getTranslation('chat.tool_selector.select_all')" @click="toggleHead(head)">
												<div
													class="flex h-4 w-4 items-center justify-center rounded border transition-colors"
													:class="headSelectionState(head) === 'unchecked' ? 'border-input bg-background' : 'border-primary bg-primary text-primary-foreground'"
												>
													<Check v-if="headSelectionState(head) === 'checked'" class="h-3 w-3" />
													<Minus v-else-if="headSelectionState(head) === 'indeterminate'" class="h-3 w-3" />
												</div>
											</ShadButton>
											<ShadCollapsibleTrigger as-child>
												<ShadButton type="button" class="flex min-w-0 flex-1 items-center gap-1.5 text-left">
													<ChevronRight class="h-3 w-3 shrink-0 text-muted-foreground transition-transform" :class="isExpanded(head.id) ? 'rotate-90' : ''" />
													<div class="min-w-0 flex-1">
														<div class="flex items-center justify-between gap-2">
															<span class="truncate text-xs font-medium text-foreground">{{ head.label }}</span>
															<span class="shrink-0 text-[10px] text-muted-foreground">{{ head.selectedCount }}/{{ head.items.length }}</span>
														</div>
														<p v-if="head.description" class="truncate text-[11px] text-muted-foreground">{{ head.description }}</p>
													</div>
												</ShadButton>
											</ShadCollapsibleTrigger>
										</div>
										<ShadCollapsibleContent>
											<div class="ml-6 border-l border-border pb-1 pl-1">
												<button
													v-for="item in head.items"
													:key="item.id"
													type="button"
													class="group flex w-full items-start gap-2 rounded-md px-2 py-1 text-left transition-colors hover:bg-muted"
													@click="toggleTool(item.id)"
												>
													<div
														class="mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center rounded border transition-colors"
														:class="isSelected(item.id) ? 'border-primary bg-primary text-primary-foreground' : 'border-input bg-background'"
													>
														<Check v-if="isSelected(item.id)" class="h-3 w-3" />
													</div>
													<div class="min-w-0 flex-1">
														<span class="block truncate text-[11px] font-medium text-foreground">{{ item.name }}</span>
														<p v-if="item.description" class="truncate text-[10px] text-muted-foreground">{{ item.description }}</p>
													</div>
												</button>
											</div>
										</ShadCollapsibleContent>
									</div>
								</ShadCollapsible>
							</template>
						</template>
					</template>
					<div v-else class="flex flex-col items-center gap-1 px-3 py-6 text-center">
						<Wrench class="h-6 w-6 text-muted-foreground/40" />
						<p class="text-xs font-medium text-foreground">{{ store.getTranslation('chat.tool_selector.no_tools') }}</p>
						<p class="text-[11px] text-muted-foreground">{{ store.getTranslation('chat.tool_selector.no_tools_hint') }}</p>
					</div>
				</div>

				<div class="flex items-center gap-1 border-t border-border p-1">
					<ShadButton
						v-if="allToolIds.length > 0"
						type="button"
						class="flex-1 rounded-md px-2 py-1.5 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
						@click="selectAll"
					>
						{{ store.getTranslation('chat.tool_selector.select_all') }}
					</ShadButton>
					<ShadButton
						v-if="chatStore.enabledTools.length > 0"
						type="button"
						class="flex-1 rounded-md px-2 py-1.5 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
						@click="clearAll"
					>
						{{ store.getTranslation('chat.tool_selector.clear') }}
					</ShadButton>
				</div>
				<div class="border-t border-border p-1">
					<ShadButton
						type="button"
						class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
						@click="openMcpManager"
					>
						<Server class="h-3.5 w-3.5" />
						{{ store.getTranslation('mcp.tool_selector.manage') }}
					</ShadButton>
				</div>
			</div>
		</ShadPopoverContent>
	</ShadPopover>
</template>

<script setup lang="ts">
import {computed, onMounted, ref, watch} from 'vue';
import {Check, ChevronRight, Minus, Search, Server, Wrench, Sparkles, Globe, Code} from 'lucide-vue-next';
import {useChatStore} from '~/stores/chatStore';
import {useMainStore} from '~/stores';
import {cn} from '~/lib/utils';

const store = useMainStore();

const props = defineProps<{
	class?: string;
}>();

const chatStore = useChatStore();
const search = ref('');
const open = ref(false);

interface ToolFunction {
	id?: string;
	name: string;
	description?: string;
}

interface ToolItem {
	id: string;
	name: string;
	display_name?: string;
	description?: string;
	source_kind: string;
	mcp_server_id?: string | null;
	mcp_server_name?: string | null;
	functions?: ToolFunction[];
}

interface SubItem {
	id: string;
	name: string;
	description?: string;
}

type SelectionState = 'checked' | 'unchecked' | 'indeterminate';

interface Head {
	id: string;
	label: string;
	description?: string;
	sourceKind: string;
	items: SubItem[];
	itemCount: number;
	collapsible: boolean;
	selectedCount: number;
}

const availableTools = ref<ToolItem[]>([]);
const expandedState = ref<Record<string, boolean>>({});

const hasToolSupport = computed(() => chatStore.selectedModel?.capabilities.includes('TOOLS'));

function isExpanded(headId: string): boolean {
	if (search.value) return true;
	return expandedState.value[headId] ?? false;
}

function setExpanded(headId: string, value: boolean) {
	expandedState.value = {...expandedState.value, [headId]: value};
}

const filteredTools = computed<ToolItem[]>(() => {
	if (!search.value) return availableTools.value;
	const query = search.value.toLowerCase();
	return availableTools.value.filter(
		tool =>
			tool.name.toLowerCase().includes(query) ||
			tool.display_name?.toLowerCase().includes(query) ||
			tool.description?.toLowerCase().includes(query) ||
			tool.mcp_server_id?.toLowerCase().includes(query) ||
			(tool.functions ?? []).some(fn => fn.name.toLowerCase().includes(query) || fn.description?.toLowerCase().includes(query))
	);
});

const allToolIds = computed(() => availableTools.value.map(t => t.id));

const sourceMeta: Record<string, {label: string; icon: any}> = {
	BUILTIN: {label: 'Builtin', icon: Sparkles},
	HTTP: {label: 'HTTP', icon: Globe},
	MCP: {label: 'MCP', icon: Server},
	WASM: {label: 'WASM', icon: Code},
};

function serverNameFor(tool: ToolItem): string | undefined {
	if (tool.mcp_server_name) return tool.mcp_server_name;
	if (!tool.mcp_server_id) return undefined;
	return chatStore.userMcpServers.find(s => s.id === tool.mcp_server_id)?.name;
}

function selectedCountFor(items: SubItem[]): number {
	return items.filter(item => chatStore.enabledTools.includes(item.id)).length;
}

function buildHeads(tools: ToolItem[]): Head[] {
	const heads: Head[] = [];

	for (const tool of tools) {
		const isMcp = tool.source_kind === 'MCP' && tool.mcp_server_id;
		if (isMcp) {
			const existing = heads.find(h => h.id === `mcp:${tool.mcp_server_id}`);
			const subItem: SubItem = {
				id: tool.id,
				name: tool.display_name || tool.name,
				description: tool.description,
			};
			if (existing) {
				existing.items.push(subItem);
				existing.itemCount = existing.items.length;
			} else {
				const serverName = serverNameFor(tool);
				heads.push({
					id: `mcp:${tool.mcp_server_id}`,
					label: serverName ?? store.getTranslation('chat.tool_selector.unknown_server'),
					description: undefined,
					sourceKind: 'MCP',
					items: [subItem],
					itemCount: 1,
					collapsible: true,
					selectedCount: 0,
				});
			}
		} else {
			heads.push({
				id: `tool:${tool.id}`,
				label: tool.display_name || tool.name,
				description: tool.description,
				sourceKind: tool.source_kind || 'HTTP',
				items: [{id: tool.id, name: tool.display_name || tool.name, description: tool.description}],
				itemCount: tool.functions?.length ?? 1,
				collapsible: false,
				selectedCount: 0,
			});
		}
	}

	for (const head of heads) {
		head.selectedCount = selectedCountFor(head.items);
	}

	return heads;
}

const groupedHeads = computed(() => {
	const heads = buildHeads(filteredTools.value);
	const groups = new Map<string, Head[]>();
	for (const head of heads) {
		const kind = head.sourceKind || 'HTTP';
		if (!groups.has(kind)) groups.set(kind, []);
		groups.get(kind)!.push(head);
	}
	const order = ['BUILTIN', 'MCP', 'HTTP', 'WASM'];
	return order
		.filter(kind => groups.has(kind))
		.map(kind => {
			const meta = sourceMeta[kind] ?? {label: kind, icon: Wrench};
			return {kind, label: meta.label, icon: meta.icon, heads: groups.get(kind)!};
		});
});

function headSelectionState(head: Head): SelectionState {
	if (head.selectedCount === 0) return 'unchecked';
	if (head.selectedCount === head.items.length) return 'checked';
	return 'indeterminate';
}

function isSelected(toolId: string): boolean {
	return chatStore.enabledTools.includes(toolId);
}

function toggleTool(toolId: string) {
	chatStore.toggleTool(toolId);
}

function toggleHead(head: Head) {
	const allSelected = head.selectedCount === head.items.length;
	for (const item of head.items) {
		const selected = isSelected(item.id);
		if (allSelected && selected) chatStore.toggleTool(item.id);
		else if (!allSelected && !selected) chatStore.toggleTool(item.id);
	}
}

function selectAll() {
	for (const id of allToolIds.value) {
		if (!isSelected(id)) chatStore.toggleTool(id);
	}
}

function clearAll() {
	for (const id of availableTools.value.map(t => t.id)) {
		if (isSelected(id)) chatStore.toggleTool(id);
	}
}

function openMcpManager() {
	open.value = false;
	chatStore.mcpManagerOpen = true;
}

async function loadTools() {
	try {
		const {$customFetch} = useNuxtApp();
		const fetchedTools = await $customFetch('/api/v1/tools');
		if (Array.isArray(fetchedTools)) {
			availableTools.value = fetchedTools.map((tool: any) => ({
				id: tool.id,
				name: tool.name,
				display_name: tool.display_name,
				description: tool.description,
				source_kind: tool.source_kind || 'HTTP',
				mcp_server_id: tool.mcp_server_id ?? null,
				mcp_server_name: tool.mcp_server_name ?? null,
				functions: tool.functions,
			}));
		}
		if (!chatStore.userMcpServers || chatStore.userMcpServers.length === 0) {
			await chatStore.fetchUserMcpServers();
		}
	} catch (error) {
		console.error('Failed to load tools:', error);
	}
}

watch(
	() => chatStore.mcpManagerOpen,
	(isOpen, wasOpen) => {
		if (wasOpen && !isOpen) {
			loadTools();
			chatStore.fetchUserMcpServers();
		}
	}
);

onMounted(loadTools);
</script>
