<template>
	<div class="max-w-4xl lg:max-h-[calc(100dvh-12rem)] lg:overflow-y-auto px-3 py-2">
		<div class="flex flex-row items-center justify-between">
			<div class="mb-6">
				<h2 class="text-lg font-semibold text-foreground">Tools</h2>
				<p class="text-sm text-muted-foreground">Manage custom tools that AI models can use during conversations</p>
			</div>
			<ShadButton variant="default" size="sm" class="gap-2" @click="openCreateDialog">
				<Plus class="h-4 w-4" />
				<span>Add Tool</span>
			</ShadButton>
		</div>

		<!-- Loading state -->
		<div v-if="loading" class="flex items-center justify-center py-12">
			<Loader2 class="h-6 w-6 animate-spin text-muted-foreground" />
		</div>

		<!-- Empty state -->
		<div v-else-if="displayTools.length === 0" class="rounded-lg border border-dashed border-border bg-muted/20 p-12 text-center">
			<Wrench class="h-12 w-12 mx-auto text-muted-foreground/50 mb-4" />
			<h3 class="text-lg font-medium text-foreground mb-2">No tools yet</h3>
			<p class="text-sm text-muted-foreground mb-4">Create custom tools to extend AI capabilities</p>
			<ShadButton variant="outline" size="sm" class="gap-2" @click="openCreateDialog">
				<Plus class="h-4 w-4" />
				<span>Create your first tool</span>
			</ShadButton>
		</div>

		<!-- Tools list -->
		<div v-else class="space-y-3">
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
									Template
								</span>
								<span
									v-else-if="tool.has_user_settings"
									class="inline-flex items-center rounded-full bg-green-500/10 px-2 py-0.5 text-xs font-medium text-green-500"
								>
									Configured
								</span>
							</div>
							<p class="text-sm text-muted-foreground truncate">{{ tool.description || 'No description' }}</p>
						</div>
					</div>
					<div class="shrink-0 flex items-center gap-2">
						<template v-if="tool.is_template">
							<ShadButton variant="default" size="sm" class="gap-2" @click="configureTemplate(tool)">
								<Plus class="h-4 w-4" />
								Configure
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
							<Switch :modelValue="tool.is_enabled" @update:modelValue="(val: boolean) => toggleTool(tool, val)" />
						</template>
					</div>
				</div>
			</div>
		</div>

		<!-- Create/Edit Tool Dialog -->
		<Dialog v-model:open="dialogOpen">
			<DialogContent class="sm:max-w-[600px]">
				<DialogHeader>
					<DialogTitle class="flex items-center gap-3">
						<Wrench class="h-5 w-5 text-primary" />
						<span>{{ editingTool ? 'Edit Tool' : 'Create Tool' }}</span>
					</DialogTitle>
					<DialogDescription>
						{{ editingTool ? 'Update your tool configuration' : 'Define a new tool for AI to use' }}
					</DialogDescription>
				</DialogHeader>

				<div class="space-y-4 py-4 max-h-[60vh] overflow-y-auto">
					<div class="grid grid-cols-2 gap-4">
						<div class="space-y-2">
							<Label for="tool-name">Name (identifier)</Label>
							<Input id="tool-name" v-model="toolForm.name" type="text" placeholder="fetch_website" />
						</div>
						<div class="space-y-2">
							<Label for="tool-display-name">Display Name</Label>
							<Input id="tool-display-name" v-model="toolForm.display_name" type="text" placeholder="Fetch Website" />
						</div>
					</div>

					<div class="space-y-2">
						<Label for="tool-description">Description</Label>
						<Textarea id="tool-description" v-model="toolForm.description" placeholder="Fetches content from a URL and returns the HTML" rows="2" />
					</div>

					<div class="space-y-2">
						<Label>Source Type</Label>
						<div class="grid grid-cols-4 gap-2">
							<button
								v-for="kind in sourceKinds"
								:key="kind.value"
								type="button"
								class="flex flex-col items-center gap-2 rounded-lg border p-3 transition-all"
								:class="toolForm.source_kind === kind.value ? 'border-primary bg-primary/5' : 'border-border hover:border-border/80'"
								@click="toolForm.source_kind = kind.value"
							>
								<component :is="kind.icon" class="h-5 w-5" :class="toolForm.source_kind === kind.value ? 'text-primary' : 'text-muted-foreground'" />
								<span class="text-xs font-medium">{{ kind.label }}</span>
							</button>
						</div>
					</div>

					<!-- Source-specific config -->
					<div v-if="toolForm.source_kind === 'HTTP'" class="space-y-4 rounded-lg border border-border p-4">
						<h4 class="font-medium text-sm">HTTP Configuration</h4>
						<div class="grid grid-cols-4 gap-4">
							<div class="space-y-2">
								<Label>Method</Label>
								<ShadSelect v-model="httpConfig.method">
									<ShadSelectTrigger><ShadSelectValue /></ShadSelectTrigger>
									<ShadSelectContent>
										<ShadSelectItem value="GET">GET</ShadSelectItem>
										<ShadSelectItem value="POST">POST</ShadSelectItem>
										<ShadSelectItem value="PUT">PUT</ShadSelectItem>
									</ShadSelectContent>
								</ShadSelect>
							</div>
							<div class="col-span-3 space-y-2">
								<Label>URL</Label>
								<Input v-model="httpConfig.url" placeholder="https://api.example.com/{{input.query}}" />
							</div>
						</div>
						<div class="space-y-2">
							<Label>Headers (JSON)</Label>
							<Textarea
								v-model="httpConfig.headers_json"
								placeholder='{"Authorization": "Bearer {{settings.api_key}}"}'
								rows="2"
								class="font-mono text-sm"
							/>
						</div>
					</div>

					<div v-if="toolForm.source_kind === 'BUILTIN'" class="space-y-4 rounded-lg border border-border p-4">
						<h4 class="font-medium text-sm">Builtin Tool</h4>
						<div class="space-y-2">
							<Label>Builtin ID</Label>
							<ShadSelect v-model="builtinConfig.builtin_id">
								<ShadSelectTrigger><ShadSelectValue /></ShadSelectTrigger>
								<ShadSelectContent>
									<ShadSelectItem value="exa_search">Exa Search</ShadSelectItem>
								</ShadSelectContent>
							</ShadSelect>
						</div>
					</div>

					<div v-if="toolForm.source_kind === 'WASM'" class="space-y-4 rounded-lg border border-border p-4">
						<h4 class="font-medium text-sm">WASM Plugin</h4>
						<div class="space-y-2">
							<Label>WASM File</Label>
							<div class="flex gap-2">
								<Input type="file" accept=".wasm" @change="handleWasmUpload" :disabled="uploading" />
							</div>
							<div v-if="wasmConfig.blob_id" class="text-xs text-green-500 flex items-center gap-1">
								<Check class="h-3 w-3" />
								WASM uploaded ({{ wasmConfig.size_bytes }} bytes)
							</div>
							<div v-if="uploading" class="text-xs text-muted-foreground flex items-center gap-1">
								<Loader2 class="h-3 w-3 animate-spin" />
								Uploading...
							</div>
						</div>
						<div class="space-y-2">
							<Label>Entry Point Function</Label>
							<Input v-model="wasmConfig.entrypoint" placeholder="execute" />
							<p class="text-xs text-muted-foreground">Name of the exported function to call</p>
						</div>
					</div>

					<div v-if="toolForm.source_kind === 'MCP'" class="space-y-4 rounded-lg border border-border p-4">
						<h4 class="font-medium text-sm">MCP Server</h4>
						<div class="space-y-2">
							<Label>Transport</Label>
							<ShadSelect v-model="mcpConfig.transport">
								<ShadSelectTrigger><ShadSelectValue /></ShadSelectTrigger>
								<ShadSelectContent>
									<ShadSelectItem value="stdio">Stdio (Local Process)</ShadSelectItem>
									<ShadSelectItem value="sse">SSE (HTTP)</ShadSelectItem>
								</ShadSelectContent>
							</ShadSelect>
						</div>
						<div v-if="mcpConfig.transport === 'stdio'" class="space-y-2">
							<Label>Command</Label>
							<Input v-model="mcpConfig.command" placeholder="npx" />
							<Label>Arguments (comma-separated)</Label>
							<Input v-model="mcpConfig.args" placeholder="-y,@modelcontextprotocol/server-filesystem" />
						</div>
						<div v-if="mcpConfig.transport === 'sse'" class="space-y-2">
							<Label>URL</Label>
							<Input v-model="mcpConfig.url" placeholder="http://localhost:3001/mcp" />
							<Label>Headers (JSON)</Label>
							<Textarea v-model="mcpConfig.headers_json" placeholder='{"Authorization": "Bearer token"}' rows="2" class="font-mono text-sm" />
						</div>
						<div class="space-y-2">
							<Label>Tool Name (from server)</Label>
							<Input v-model="mcpConfig.tool_name" placeholder="read_file" />
						</div>
					</div>

					<div class="space-y-2">
						<Label>Input Schema</Label>
						<SchemaBuilder v-model="toolForm.input_schema_json" />
						<p class="text-xs text-muted-foreground">Define what parameters the AI should provide</p>
					</div>

					<div class="space-y-2">
						<Label>Settings Schema (optional)</Label>
						<SchemaBuilder v-model="toolForm.settings_schema_json" :is-settings="true" />
						<p class="text-xs text-muted-foreground">Define user settings like API keys</p>
					</div>

					<div class="flex items-center gap-4">
						<div class="flex items-center gap-2">
							<Switch v-model:checked="toolForm.is_enabled" />
							<Label>Enabled</Label>
						</div>
						<div class="flex items-center gap-2">
							<Switch v-model:checked="toolForm.is_public" />
							<Label>Public</Label>
						</div>
					</div>
				</div>

				<DialogFooter class="gap-2 sm:gap-0">
					<ShadButton v-if="editingTool" variant="destructive" @click="deleteTool" :disabled="saving" class="mr-auto">
						<Trash2 class="h-4 w-4 mr-2" />
						Delete
					</ShadButton>
					<div class="flex flex-row gap-2">
						<ShadButton variant="outline" @click="dialogOpen = false">Cancel</ShadButton>
						<ShadButton @click="saveTool" :disabled="saving">
							<Loader2 v-if="saving" class="h-4 w-4 animate-spin mr-2" />
							Save
						</ShadButton>
					</div>
				</DialogFooter>
			</DialogContent>
		</Dialog>

		<!-- Settings Dialog -->
		<Dialog v-model:open="settingsDialogOpen">
			<DialogContent class="sm:max-w-[400px]">
				<DialogHeader>
					<DialogTitle class="flex items-center gap-3">
						<Key class="h-5 w-5 text-primary" />
						<span>Tool Settings</span>
					</DialogTitle>
					<DialogDescription> Configure your personal settings for {{ settingsTool?.display_name || settingsTool?.name }} </DialogDescription>
				</DialogHeader>

				<div class="space-y-4 py-4">
					<div v-for="(field, key) in settingsFields" :key="key" class="space-y-2">
						<Label :for="`setting-${key}`">{{ field.title || key }}</Label>
						<Input :id="`setting-${key}`" v-model="userSettings[key]" :type="field.secret ? 'password' : 'text'" :placeholder="field.description || ''" />
					</div>
				</div>

				<DialogFooter>
					<ShadButton variant="outline" @click="settingsDialogOpen = false">Cancel</ShadButton>
					<ShadButton @click="saveSettings" :disabled="saving">
						<Loader2 v-if="saving" class="h-4 w-4 animate-spin mr-2" />
						Save
					</ShadButton>
				</DialogFooter>
			</DialogContent>
		</Dialog>

		<!-- Test Tool Dialog -->
		<ToolTestDialog v-model:open="testDialogOpen" :tool="testingTool" />
	</div>
</template>

<script setup lang="ts">
import {ref, reactive, onMounted, computed} from 'vue';
import {Plus, Settings2, Loader2, Wrench, Globe, Code, Server, Sparkles, Key, Play, Trash2, Check} from 'lucide-vue-next';
import SchemaBuilder from '@/components/settings/SchemaBuilder.vue';
import ToolTestDialog from '@/components/settings/ToolTestDialog.vue';
import {useMainStore} from '@/stores';
import {Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle} from '@/components/ui/dialog';
import {Input} from '@/components/ui/input';
import {Textarea} from '@/components/ui/textarea';
import {Label} from '@/components/ui/label';
import {Switch} from '@/components/ui/switch';

const {$customFetch} = useNuxtApp();
const store = useMainStore();

interface Tool {
	id: string;
	name: string;
	display_name?: string;
	description?: string;
	icon?: string;
	source_kind: string;
	source_config: any;
	input_schema: any;
	settings_schema?: any;
	is_enabled: boolean;
	is_public: boolean;
	has_user_settings?: boolean;
}

const loading = ref(true);
const saving = ref(false);
const uploading = ref(false);
const tools = ref<Tool[]>([]);
const dialogOpen = ref(false);
const settingsDialogOpen = ref(false);
const editingTool = ref<Tool | null>(null);
const settingsTool = ref<Tool | null>(null);
const userSettings = ref<Record<string, string>>({});
const testDialogOpen = ref(false);
const testingTool = ref<Tool | null>(null);

const sourceKinds = [
	{value: 'BUILTIN', label: 'Builtin', icon: Sparkles},
	{value: 'HTTP', label: 'HTTP', icon: Globe},
	{value: 'WASM', label: 'WASM', icon: Code},
	{value: 'MCP', label: 'MCP', icon: Server},
];

// Builtin tool templates - these are pre-configured tools users can easily add
const builtinToolTemplates = [
	{
		name: 'exa_search',
		display_name: 'Exa Search',
		description: 'Search the web using Exa AI for current and accurate information',
		source_kind: 'BUILTIN',
		icon: Globe,
		source_config: {builtin_id: 'exa_search'},
		input_schema: {
			type: 'object',
			properties: {
				query: {type: 'string', description: 'Search query'},
				num_results: {type: 'integer', description: 'Number of results (1-10)', default: 5},
			},
			required: ['query'],
		},
		settings_schema: {
			type: 'object',
			properties: {
				api_key: {type: 'string', title: 'Exa API Key', secret: true, description: 'Get your API key from exa.ai'},
			},
			required: ['api_key'],
		},
	},
];

const displayTools = computed(() => {
	const result: any[] = [...tools.value];

	console.log('Displaying tools:', builtinToolTemplates);
	// Add unconfigured builtin templates
	for (const template of builtinToolTemplates) {
		const exists = tools.value.some(t => t.name === template.name);
		if (!exists) {
			result.push({
				...template,
				id: `template_${template.name}`,
				is_enabled: false,
				is_public: false,
				is_template: true,
			});
		}
	}

	return result;
});

const toolForm = reactive({
	name: '',
	display_name: '',
	description: '',
	source_kind: 'HTTP',
	input_schema_json: '',
	settings_schema_json: '',
	is_enabled: true,
	is_public: false,
});

const httpConfig = reactive({
	method: 'GET',
	url: '',
	headers_json: '{}',
	body_template: '',
});

const builtinConfig = reactive({
	builtin_id: 'exa_search',
});

const wasmConfig = reactive({
	blob_id: '',
	size_bytes: 0,
	entrypoint: 'execute',
});

const mcpConfig = reactive({
	transport: 'stdio',
	command: '',
	args: '',
	url: '',
	headers_json: '{}',
	tool_name: '',
});

const settingsFields = computed(() => {
	if (!settingsTool.value?.settings_schema?.properties) return {};
	return settingsTool.value.settings_schema.properties;
});

async function handleWasmUpload(event: Event) {
	const target = event.target as HTMLInputElement;
	const file = target.files?.[0];
	if (!file) return;

	uploading.value = true;
	try {
		const arrayBuffer = await file.arrayBuffer();
		const base64 = btoa(String.fromCharCode(...new Uint8Array(arrayBuffer)));

		const result = await $customFetch<{blob_id: string; size_bytes: number}>('/api/v1/tools/wasm/upload', {
			method: 'POST',
			body: {
				filename: file.name,
				content: base64,
			},
		});

		wasmConfig.blob_id = result.blob_id;
		wasmConfig.size_bytes = result.size_bytes;
		store.toast('WASM uploaded', {type: 'success'});
	} catch (e: any) {
		store.toast('WASM upload failed', {type: 'error', description: e.message});
	} finally {
		uploading.value = false;
	}
}

function getToolIcon(sourceKind: string) {
	const kind = sourceKinds.find(k => k.value === sourceKind);
	return kind?.icon || Wrench;
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
		const result = await $customFetch('/api/v1/tools');
		if (Array.isArray(result)) {
			tools.value = result;
		}
	} catch (e: any) {
		console.error('Failed to load tools:', e);
	} finally {
		loading.value = false;
	}
}

function openCreateDialog() {
	editingTool.value = null;
	Object.assign(toolForm, {
		name: '',
		display_name: '',
		description: '',
		source_kind: 'HTTP',
		input_schema_json: JSON.stringify({type: 'object', properties: {}}, null, 2),
		settings_schema_json: '',
		is_enabled: true,
		is_public: false,
	});
	Object.assign(httpConfig, {method: 'GET', url: '', headers_json: '{}', body_template: ''});
	dialogOpen.value = true;
}

async function configureTemplate(template: any) {
	// Create the tool from the template
	try {
		saving.value = true;
		const body = {
			name: template.name,
			display_name: template.display_name,
			description: template.description,
			source_kind: template.source_kind,
			source_config: template.source_config,
			input_schema: template.input_schema,
			settings_schema: template.settings_schema,
			is_enabled: true,
			is_public: false,
		};

		const created = (await $customFetch('/api/v1/tools', {method: 'POST', body})) as Tool;
		store.toast('Tool added! Please configure your API key.', {type: 'success'});
		await loadTools();

		// Open settings dialog to configure API key
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

function openEditDialog(tool: Tool) {
	editingTool.value = tool;
	Object.assign(toolForm, {
		name: tool.name,
		display_name: tool.display_name || '',
		description: tool.description || '',
		source_kind: tool.source_kind,
		input_schema_json: JSON.stringify(tool.input_schema, null, 2),
		settings_schema_json: tool.settings_schema ? JSON.stringify(tool.settings_schema, null, 2) : '',
		is_enabled: tool.is_enabled,
		is_public: tool.is_public,
	});

	if (tool.source_kind === 'HTTP' && tool.source_config) {
		Object.assign(httpConfig, {
			method: tool.source_config.method || 'GET',
			url: tool.source_config.url || '',
			headers_json: JSON.stringify(tool.source_config.headers || {}, null, 2),
			body_template: tool.source_config.body_template || '',
		});
	}
	if (tool.source_kind === 'BUILTIN' && tool.source_config) {
		builtinConfig.builtin_id = tool.source_config.builtin_id || 'exa_search';
	}

	dialogOpen.value = true;
}

async function openSettingsDialog(tool: Tool) {
	settingsTool.value = tool;
	try {
		const settings = (await $customFetch(`/api/v1/tools/${tool.id}/settings`)) as Record<string, string>;
		userSettings.value = settings || {};
	} catch {
		userSettings.value = {} as Record<string, string>;
	}
	settingsDialogOpen.value = true;
}

async function saveTool() {
	saving.value = true;
	try {
		let source_config: any = {};
		if (toolForm.source_kind === 'HTTP') {
			source_config = {
				method: httpConfig.method,
				url: httpConfig.url,
				headers: JSON.parse(httpConfig.headers_json || '{}'),
				body_template: httpConfig.body_template || null,
			};
		} else if (toolForm.source_kind === 'BUILTIN') {
			source_config = {builtin_id: builtinConfig.builtin_id};
		} else if (toolForm.source_kind === 'WASM') {
			source_config = {
				wasm_blob_id: wasmConfig.blob_id,
				entrypoint: wasmConfig.entrypoint || 'execute',
			};
		} else if (toolForm.source_kind === 'MCP') {
			if (mcpConfig.transport === 'stdio') {
				source_config = {
					transport: 'stdio',
					command: mcpConfig.command,
					args: mcpConfig.args
						.split(',')
						.map(s => s.trim())
						.filter(Boolean),
					tool_name: mcpConfig.tool_name,
				};
			} else {
				source_config = {
					transport: 'sse',
					url: mcpConfig.url,
					headers: JSON.parse(mcpConfig.headers_json || '{}'),
					tool_name: mcpConfig.tool_name,
				};
			}
		}

		const body = {
			name: toolForm.name,
			display_name: toolForm.display_name || null,
			description: toolForm.description || null,
			source_kind: toolForm.source_kind,
			source_config,
			input_schema: JSON.parse(toolForm.input_schema_json || '{}'),
			settings_schema: toolForm.settings_schema_json ? JSON.parse(toolForm.settings_schema_json) : null,
			is_enabled: toolForm.is_enabled,
			is_public: toolForm.is_public,
		};

		if (editingTool.value) {
			await $customFetch(`/api/v1/tools/${editingTool.value.id}`, {method: 'PUT', body});
		} else {
			await $customFetch('/api/v1/tools', {method: 'POST', body});
		}

		store.toast('Tool saved successfully', {type: 'success'});
		dialogOpen.value = false;
		await loadTools();
	} catch (e: any) {
		store.toast('Failed to save tool', {type: 'error', description: e.message});
	} finally {
		saving.value = false;
	}
}

async function deleteTool() {
	if (!editingTool.value) return;
	saving.value = true;
	try {
		await $customFetch(`/api/v1/tools/${editingTool.value.id}`, {method: 'DELETE'});
		store.toast('Tool deleted', {type: 'success'});
		dialogOpen.value = false;
		await loadTools();
	} catch (e: any) {
		store.toast('Failed to delete tool', {type: 'error', description: e.message});
	} finally {
		saving.value = false;
	}
}

async function toggleTool(tool: Tool, enabled: boolean) {
	try {
		await $customFetch(`/api/v1/tools/${tool.id}`, {
			method: 'PUT',
			body: {is_enabled: enabled},
		});
		tool.is_enabled = enabled;
	} catch (e: any) {
		store.toast('Failed to toggle tool', {type: 'error'});
	}
}

async function saveSettings() {
	if (!settingsTool.value) return;
	saving.value = true;
	try {
		await $customFetch(`/api/v1/tools/${settingsTool.value.id}/settings`, {
			method: 'PUT',
			body: {settings: userSettings.value},
		});
		store.toast('Settings saved', {type: 'success'});
		settingsDialogOpen.value = false;
		await loadTools();
	} catch (e: any) {
		store.toast('Failed to save settings', {type: 'error', description: e.message});
	} finally {
		saving.value = false;
	}
}

function testTool(tool: Tool) {
	testingTool.value = tool;
	testDialogOpen.value = true;
}

onMounted(() => {
	loadTools();
});
</script>
