<template>
	<SettingsPage :title="store.getTranslation('settings.tools.title')" :description="store.getTranslation('settings.tools.description')">
		<template #actions>
			<ShadButton variant="outline" size="sm" class="gap-2" @click="mcpManagerOpen = true">
				<Server class="h-4 w-4" />
				<span>{{ store.getTranslation('sidebar.manage_mcp') }}</span>
			</ShadButton>
			<ShadButton variant="default" size="sm" class="gap-2" @click="openCreateDialog">
				<Plus class="h-4 w-4" />
				<span>{{ store.getTranslation('settings.tools.add') }}</span>
			</ShadButton>
		</template>

		<McpManagerDialog v-model:open="mcpManagerOpen" :admin="true" @changed="loadTools" />
		<McpServerEditDialog v-model:open="mcpServerEditOpen" :server="editingMcpServer" :admin="true" @changed="loadTools" />

		<div v-if="loading" class="flex items-center justify-center py-12">
			<Loader2 class="h-6 w-6 animate-spin text-muted-foreground" />
		</div>

		<div v-else class="space-y-4">
			<SettingsToolsMcpGroups v-if="mcpServerGroups.length > 0" :groups="mcpServerGroups" @edit="openMcpServerEdit" @toggle="toggleTool" />

			<div v-else-if="displayTools.length === 0 && mcpServerGroups.length === 0" class="rounded-lg border border-dashed border-border bg-muted/20 p-12 text-center">
				<Wrench class="h-12 w-12 mx-auto text-muted-foreground/50 mb-4" />
				<h3 class="text-lg font-medium text-foreground mb-2">{{ store.getTranslation('settings.tools.no_tools') }}</h3>
				<p class="text-sm text-muted-foreground mb-4">{{ store.getTranslation('settings.tools.no_tools_description') }}</p>
				<ShadButton variant="outline" size="sm" class="gap-2" @click="openCreateDialog">
					<Plus class="h-4 w-4" />
					<span>{{ store.getTranslation('settings.tools.create_first') }}</span>
				</ShadButton>
			</div>

			<div v-if="displayTools.length > 0" class="space-y-3">
				<div
					v-for="tool in displayTools"
					:key="tool.id"
					class="rounded-lg border p-4 transition-all"
					:class="tool.is_template ? 'border-dashed border-primary/30 bg-primary/5 hover:border-primary/50' : 'border-border bg-card hover:border-border/80'"
				>
					<div class="flex items-center justify-between gap-4">
						<div class="flex items-center gap-4 min-w-0">
							<div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg" :class="tool.is_template ? 'bg-primary/20' : 'bg-primary/10'">
								<component :is="getToolIcon(tool.source_kind)" class="h-5 w-5 text-primary" />
							</div>
							<div class="min-w-0">
								<div class="flex items-center gap-2">
									<h3 class="font-medium text-foreground">{{ tool.display_name || tool.name }}</h3>
									<span class="inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium" :class="getSourceBadgeClass(tool.source_kind)">
										{{ tool.source_kind }}
									</span>
									<span v-if="tool.is_template" class="inline-flex items-center rounded-full bg-primary/10 px-2 py-0.5 text-xs font-medium text-primary">
										{{ store.getTranslation('settings.tools.template') }}
									</span>
									<span
										v-else-if="tool.has_user_settings"
										class="inline-flex items-center rounded-full bg-green-500/10 px-2 py-0.5 text-xs font-medium text-green-500"
									>
										{{ store.getTranslation('settings.tools.configured') }}
									</span>
								</div>
								<p class="text-sm text-muted-foreground truncate">{{ tool.description || store.getTranslation('settings.tools.no_description') }}</p>
							</div>
						</div>
						<div class="shrink-0 flex items-center gap-2">
							<template v-if="tool.is_template">
								<ShadButton variant="default" size="sm" class="gap-2" @click="configureTemplate(tool)">
									<Plus class="h-4 w-4" />
									{{ store.getTranslation('settings.tools.configure') }}
								</ShadButton>
							</template>
							<template v-else>
								<ShadButton v-if="tool.settings_schema" variant="outline" size="sm" class="gap-2" @click="openSettingsDialog(tool)">
									<Key class="h-4 w-4" />
								</ShadButton>
								<ShadButton variant="outline" size="sm" class="gap-2" @click="openEditDialog(tool)">
									<Settings2 class="h-4 w-4" />
								</ShadButton>
								<ShadButton variant="outline" size="sm" class="gap-2" @click="testTool(tool)">
									<Play class="h-4 w-4" />
								</ShadButton>
								<ShadSwitch :modelValue="tool.is_enabled" @update:modelValue="(val: boolean) => toggleTool(tool, val)" />
							</template>
						</div>
					</div>
				</div>
			</div>
		</div>

		<SettingsToolsEditDialog v-model:open="dialogOpen" :tool="editingTool" @saved="loadTools" />
		<SettingsToolsSettingsDialog v-model:open="settingsDialogOpen" :tool="settingsTool" @saved="loadTools" />
		<ToolTestDialog v-model:open="testDialogOpen" :tool="testingTool" />
	</SettingsPage>
</template>

<script setup lang="ts">
import {Plus, Settings2, Loader2, Wrench, Globe, Code, Server, Sparkles, Key, Play} from 'lucide-vue-next';
import ToolTestDialog from '@/components/settings/ToolTestDialog.vue';
import McpManagerDialog from '@/components/mcp/McpManagerDialog.vue';
import McpServerEditDialog from '@/components/mcp/McpServerEditDialog.vue';
import {useMainStore} from '@/stores';
import type {Tool} from '~/types/tools';

const store = useMainStore();
const builtinToolTemplates = useBuiltinToolTemplates();

const loading = ref(true);
const saving = ref(false);
const tools = ref<Tool[]>([]);
const mcpManagerOpen = ref(false);
const mcpServerEditOpen = ref(false);
const editingMcpServer = ref<any | null>(null);
const dialogOpen = ref(false);
const settingsDialogOpen = ref(false);
const editingTool = ref<Tool | null>(null);
const settingsTool = ref<Tool | null>(null);
const testDialogOpen = ref(false);
const testingTool = ref<Tool | null>(null);

const sourceIcons: Record<string, any> = {BUILTIN: Sparkles, HTTP: Globe, WASM: Code, MCP: Server};

const mcpServerGroups = computed(() => {
	const groups = new Map<string, {server_id: string; server_name: string; tools: Tool[]}>();
	for (const tool of tools.value) {
		if (!tool.mcp_server_id) continue;
		const group = groups.get(tool.mcp_server_id) ?? {server_id: tool.mcp_server_id, server_name: tool.mcp_server_name || tool.mcp_server_id, tools: []};
		group.tools.push(tool);
		groups.set(tool.mcp_server_id, group);
	}
	return Array.from(groups.values());
});

const displayTools = computed(() => {
	const result: any[] = tools.value.filter(t => !t.mcp_server_id);

	for (const template of builtinToolTemplates.value) {
		if (!tools.value.some(t => t.name === template.name)) {
			result.push({
				...template,
				id: `template_${template.name}`,
				input_schema: template.functions[0]?.input_schema || {},
				is_enabled: false,
				is_template: true,
			});
		}
	}

	return result;
});

async function openMcpServerEdit(serverId: string) {
	try {
		editingMcpServer.value = await mcpServer(serverId);
		mcpServerEditOpen.value = true;
	} catch (e: any) {
		store.toast('Failed to load MCP server', {type: 'error'});
	}
}

function getToolIcon(sourceKind: string) {
	return sourceIcons[sourceKind] || Wrench;
}

function getSourceBadgeClass(sourceKind: string) {
	switch (sourceKind) {
		case 'BUILTIN':
			return 'bg-purple-500/10 text-purple-500';
		case 'HTTP':
			return 'bg-blue-500/10 text-blue-500';
		case 'WASM':
			return 'bg-orange-500/10 text-orange-500';
		case 'MCP':
			return 'bg-green-500/10 text-green-500';
		default:
			return 'bg-muted text-muted-foreground';
	}
}

async function loadTools() {
	loading.value = true;
	try {
		const result = await adminTools();
		if (Array.isArray(result)) {
			tools.value = result.map((t: any) => ({
				...t,
				mcp_server_id: t.mcp_server_id ?? null,
				mcp_server_name: t.mcp_server_name ?? null,
			}));
		}
	} catch (e: any) {
		console.error('Failed to load tools:', e);
	} finally {
		loading.value = false;
	}
}

function openCreateDialog() {
	editingTool.value = null;
	dialogOpen.value = true;
}

function openEditDialog(tool: Tool) {
	editingTool.value = tool;
	dialogOpen.value = true;
}

function openSettingsDialog(tool: Tool) {
	settingsTool.value = tool;
	settingsDialogOpen.value = true;
}

async function configureTemplate(template: any) {
	try {
		saving.value = true;
		const body = {
			name: template.name,
			display_name: template.display_name,
			description: template.description,
			source_kind: template.source_kind,
			source_config: template.source_config,
			functions: template.functions.map((f: any) => ({
				name: f.name,
				description: f.description,
				input_schema: f.input_schema,
				entrypoint: f.entrypoint,
			})),
			settings_schema: template.settings_schema,
			is_enabled: true,
		};

		const created = (await saveAdminTool(undefined, body)) as Tool;
		store.toast('Tool added! Please configure your API key.', {type: 'success'});
		await loadTools();

		if (created.settings_schema) {
			const tool = tools.value.find(t => t.id === created.id);
			if (tool) {
				openSettingsDialog(tool);
			}
		}
	} catch (e: any) {
		store.toast('Failed to create tool', {type: 'error', description: e.message});
	} finally {
		saving.value = false;
	}
}

async function toggleTool(tool: Tool, enabled: boolean) {
	try {
		await saveAdminTool(tool.id, {is_enabled: enabled});
		tool.is_enabled = enabled;
	} catch (e: any) {
		store.toast('Failed to toggle tool', {type: 'error'});
	}
}

function testTool(tool: Tool) {
	testingTool.value = tool;
	testDialogOpen.value = true;
}

onMounted(loadTools);
</script>
