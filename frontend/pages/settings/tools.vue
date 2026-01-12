<template>
	<div class="max-w-4xl lg:max-h-[calc(100dvh-12rem)] lg:overflow-y-auto px-3 py-2">
		<div class="flex flex-row items-center justify-between">
			<div class="mb-6">
				<h2 class="text-lg font-semibold text-foreground">{{ store.getTranslation('settings.tools.title') }}</h2>
				<p class="text-sm text-muted-foreground">{{ store.getTranslation('settings.tools.description') }}</p>
			</div>
			<ShadButton variant="default" size="sm" class="gap-2" @click="openCreateDialog">
				<Plus class="h-4 w-4" />
				<span>{{ store.getTranslation('settings.tools.add') }}</span>
			</ShadButton>
		</div>

		<div v-if="loading" class="flex items-center justify-center py-12">
			<Loader2 class="h-6 w-6 animate-spin text-muted-foreground" />
		</div>

		<div v-else-if="displayTools.length === 0" class="rounded-lg border border-dashed border-border bg-muted/20 p-12 text-center">
			<Wrench class="h-12 w-12 mx-auto text-muted-foreground/50 mb-4" />
			<h3 class="text-lg font-medium text-foreground mb-2">{{ store.getTranslation('settings.tools.no_tools') }}</h3>
			<p class="text-sm text-muted-foreground mb-4">{{ store.getTranslation('settings.tools.no_tools_description') }}</p>
			<ShadButton variant="outline" size="sm" class="gap-2" @click="openCreateDialog">
				<Plus class="h-4 w-4" />
				<span>{{ store.getTranslation('settings.tools.create_first') }}</span>
			</ShadButton>
		</div>

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
							<Switch :modelValue="tool.is_enabled" @update:modelValue="(val: boolean) => toggleTool(tool, val)" />
						</template>
					</div>
				</div>
			</div>
		</div>

		<Dialog v-model:open="dialogOpen">
			<DialogContent class="sm:max-w-[550px]">
				<DialogHeader>
					<DialogTitle>{{ editingTool ? store.getTranslation('settings.tools.edit') : store.getTranslation('settings.tools.create') }}</DialogTitle>
				</DialogHeader>

				<Tabs v-model="activeTab" class="w-full">
					<TabsList class="grid w-full" :class="isBuiltinTool ? 'grid-cols-2' : 'grid-cols-3'">
						<TabsTrigger value="general">{{ store.getTranslation('settings.tools.general') }}</TabsTrigger>
						<TabsTrigger value="source">{{ store.getTranslation('settings.tools.source') }}</TabsTrigger>
						<TabsTrigger v-if="!isBuiltinTool" value="functions">{{ store.getTranslation('settings.tools.functions') }}</TabsTrigger>
					</TabsList>

					<div class="mt-4 max-h-[50vh] overflow-y-auto pr-1">
						<TabsContent value="general" class="space-y-4 mt-0">
							<div class="space-y-2">
								<Label for="tool-name">{{ store.getTranslation('settings.tools.identifier') }}</Label>
								<Input id="tool-name" v-model="toolForm.name" :placeholder="store.getTranslation('settings.tools.identifier_placeholder')" />
							</div>
							<div class="space-y-2">
								<Label for="tool-display-name">{{ store.getTranslation('settings.tools.display_name') }}</Label>
								<Input
									id="tool-display-name"
									v-model="toolForm.display_name"
									:placeholder="store.getTranslation('settings.tools.display_name_placeholder')"
								/>
							</div>
							<div class="space-y-2">
								<Label for="tool-description">{{ store.getTranslation('settings.tools.description') }}</Label>
								<Textarea
									id="tool-description"
									v-model="toolForm.description"
									:placeholder="store.getTranslation('settings.tools.description_placeholder')"
									rows="2"
								/>
							</div>
							<div class="flex items-center justify-between pt-2">
								<div class="flex items-center gap-2">
									<Switch v-model:modelValue="toolForm.is_public" />
									<Label>{{ store.getTranslation('settings.tools.public') }}</Label>
								</div>
							</div>
						</TabsContent>

						<TabsContent value="source" class="space-y-4 mt-0">
							<div class="grid grid-cols-4 gap-2">
								<button
									v-for="kind in sourceKinds"
									:key="kind.value"
									type="button"
									class="flex flex-col items-center justify-center gap-1.5 rounded-lg border p-2.5 transition-all"
									:class="[
										toolForm.source_kind === kind.value ? 'border-primary bg-primary/5' : 'border-border',
										kind.disabled || isBuiltinTool ? 'opacity-50 cursor-not-allowed' : 'hover:border-border/80',
									]"
									:disabled="kind.disabled || isBuiltinTool"
									@click="!kind.disabled && (toolForm.source_kind = kind.value)"
								>
									<component :is="kind.icon" class="h-4 w-4" :class="toolForm.source_kind === kind.value ? 'text-primary' : 'text-muted-foreground'" />
									<span class="text-xs font-medium">{{ kind.label }}</span>
									<span v-if="kind.disabled" class="text-[10px] text-muted-foreground">{{ store.getTranslation('settings.tools.soon') }}</span>
								</button>
							</div>

							<div v-if="toolForm.source_kind === 'HTTP'" class="space-y-3 pt-2">
								<div class="flex gap-2">
									<div class="w-24">
										<ShadSelect v-model="httpConfig.method">
											<ShadSelectTrigger><ShadSelectValue /></ShadSelectTrigger>
											<ShadSelectContent>
												<ShadSelectItem value="GET">GET</ShadSelectItem>
												<ShadSelectItem value="POST">POST</ShadSelectItem>
												<ShadSelectItem value="PUT">PUT</ShadSelectItem>
											</ShadSelectContent>
										</ShadSelect>
									</div>
									<Input v-model="httpConfig.url" :placeholder="store.getTranslation('settings.tools.url_placeholder')" class="flex-1" />
								</div>
								<div class="space-y-1">
									<Label class="text-xs">{{ store.getTranslation('settings.tools.headers') }}</Label>
									<Textarea
										v-model="httpConfig.headers_json"
										:placeholder="store.getTranslation('settings.tools.headers_placeholder')"
										rows="2"
										class="font-mono text-xs"
									/>
								</div>
							</div>

							<div v-if="toolForm.source_kind === 'MCP'" class="space-y-3 pt-2">
								<ShadSelect v-model="mcpConfig.transport">
									<ShadSelectTrigger><ShadSelectValue /></ShadSelectTrigger>
									<ShadSelectContent>
										<ShadSelectItem value="stdio">{{ store.getTranslation('settings.tools.stdio') }}</ShadSelectItem>
										<ShadSelectItem value="sse">{{ store.getTranslation('settings.tools.sse') }}</ShadSelectItem>
									</ShadSelectContent>
								</ShadSelect>
								<template v-if="mcpConfig.transport === 'stdio'">
									<Input v-model="mcpConfig.command" :placeholder="store.getTranslation('settings.tools.command_placeholder')" />
									<Input v-model="mcpConfig.args" :placeholder="store.getTranslation('settings.tools.args_placeholder')" />
								</template>
								<template v-if="mcpConfig.transport === 'sse'">
									<Input v-model="mcpConfig.url" :placeholder="store.getTranslation('settings.tools.url_placeholder')" />
									<Textarea
										v-model="mcpConfig.headers_json"
										:placeholder="store.getTranslation('settings.tools.headers_placeholder')"
										rows="2"
										class="font-mono text-xs"
									/>
								</template>
								<Input v-model="mcpConfig.tool_name" :placeholder="store.getTranslation('settings.tools.tool_name_placeholder')" />
							</div>

							<div v-if="!isBuiltinTool" class="space-y-2 pt-2 border-t border-border">
								<Label class="text-xs text-muted-foreground">{{ store.getTranslation('settings.tools.settings_schema') }}</Label>
								<SchemaBuilder v-model="toolForm.settings_schema_json" :is-settings="true" />
							</div>
						</TabsContent>

						<TabsContent v-if="!isBuiltinTool" value="functions" class="space-y-4 mt-0">
							<div class="flex items-center justify-between">
								<div class="flex gap-1 overflow-x-auto">
									<button
										v-for="(func, idx) in toolForm.functions"
										:key="idx"
										type="button"
										class="px-2.5 py-1 text-xs rounded-md transition-colors whitespace-nowrap"
										:class="activeFunctionIdx === idx ? 'bg-primary text-primary-foreground' : 'bg-muted hover:bg-muted/80'"
										@click="activeFunctionIdx = idx"
									>
										{{ func.name || store.getTranslation('settings.tools.new') }}
									</button>
								</div>
								<ShadButton variant="ghost" size="sm" @click="addfunction" type="button">
									<Plus class="h-3 w-3" />
								</ShadButton>
							</div>

							<div v-if="toolForm.functions[activeFunctionIdx]" class="space-y-3">
								<div class="flex gap-2">
									<div class="flex-1 space-y-1">
										<Label class="text-xs">{{ store.getTranslation('settings.tools.name') }}</Label>
										<Input
											v-model="toolForm.functions[activeFunctionIdx].name"
											:placeholder="store.getTranslation('settings.tools.function_name_placeholder')"
										/>
									</div>
									<div class="flex-1 space-y-1">
										<Label class="text-xs">{{ store.getTranslation('settings.tools.entrypoint') }}</Label>
										<Input
											v-model="toolForm.functions[activeFunctionIdx].entrypoint"
											:placeholder="store.getTranslation('settings.tools.optional')"
										/>
									</div>
								</div>
								<div class="space-y-1">
									<Label class="text-xs">{{ store.getTranslation('settings.tools.description') }}</Label>
									<Input
										v-model="toolForm.functions[activeFunctionIdx].description"
										:placeholder="store.getTranslation('settings.tools.function_description_placeholder')"
									/>
								</div>
								<div class="space-y-1">
									<Label class="text-xs">{{ store.getTranslation('settings.tools.input_schema') }}</Label>
									<SchemaBuilder v-model="toolForm.functions[activeFunctionIdx].input_schema_json" />
								</div>
								<div v-if="toolForm.functions.length > 1" class="pt-2">
									<ShadButton
										variant="ghost"
										size="sm"
										class="text-destructive hover:text-destructive h-7 text-xs"
										@click="removeFunction(activeFunctionIdx)"
										type="button"
									>
										<Trash2 class="h-3 w-3 mr-1" />
										{{ store.getTranslation('settings.tools.remove') }}
									</ShadButton>
								</div>
							</div>
						</TabsContent>
					</div>
				</Tabs>

				<DialogFooter class="gap-2 sm:gap-0 pt-2">
					<ShadButton v-if="editingTool && !isBuiltinTool" variant="destructive" size="sm" @click="deleteTool" :disabled="saving" class="mr-auto">
						<Trash2 class="h-3 w-3 mr-1" />
						{{ store.getTranslation('common.delete') }}
					</ShadButton>
					<div class="flex gap-2">
						<ShadButton variant="outline" size="sm" @click="dialogOpen = false">{{ store.getTranslation('common.cancel') }}</ShadButton>
						<ShadButton size="sm" @click="saveTool" :disabled="saving">
							<Loader2 v-if="saving" class="h-3 w-3 animate-spin mr-1" />
							{{ store.getTranslation('common.save') }}
						</ShadButton>
					</div>
				</DialogFooter>
			</DialogContent>
		</Dialog>

		<Dialog v-model:open="settingsDialogOpen">
			<DialogContent class="sm:max-w-[400px]">
				<DialogHeader>
					<DialogTitle class="flex items-center gap-3">
						<Key class="h-5 w-5 text-primary" />
						<span>{{ store.getTranslation('settings.tools.settings') }}</span>
					</DialogTitle>
					<DialogDescription>{{
						store.getTranslation('settings.tools.settings_description', {name: settingsTool?.display_name || settingsTool?.name})
					}}</DialogDescription>
				</DialogHeader>

				<div class="space-y-4 py-4">
					<div v-for="(field, key) in settingsFields" :key="key" class="space-y-2">
						<Label :for="`setting-${key}`">{{ field.title || key }}</Label>
						<template v-if="field.enum && field.enum.length > 0">
							<ShadSelect v-model="userSettings[key]">
								<ShadSelectTrigger :id="`setting-${key}`">
									<ShadSelectValue :placeholder="field.description || store.getTranslation('settings.tools.select_placeholder')" />
								</ShadSelectTrigger>
								<ShadSelectContent>
									<ShadSelectItem v-for="option in field.enum" :key="option" :value="option">
										{{ option }}
									</ShadSelectItem>
								</ShadSelectContent>
							</ShadSelect>
						</template>
						<template v-else>
							<Input :id="`setting-${key}`" v-model="userSettings[key]" :type="field.secret ? 'password' : 'text'" :placeholder="field.description || ''" />
						</template>
						<p v-if="field.description && field.enum" class="text-xs text-muted-foreground">{{ field.description }}</p>
					</div>
				</div>

				<DialogFooter>
					<ShadButton variant="outline" @click="settingsDialogOpen = false">{{ store.getTranslation('common.cancel') }}</ShadButton>
					<ShadButton @click="saveSettings" :disabled="saving">
						<Loader2 v-if="saving" class="h-4 w-4 animate-spin mr-2" />
						{{ store.getTranslation('common.save') }}
					</ShadButton>
				</DialogFooter>
			</DialogContent>
		</Dialog>

		<ToolTestDialog v-model:open="testDialogOpen" :tool="testingTool" />
	</div>
</template>

<script setup lang="ts">
import {ref, reactive, onMounted, computed} from 'vue';
import {Plus, Settings2, Loader2, Wrench, Globe, Code, Server, Sparkles, Key, Play, Trash2, Check} from 'lucide-vue-next';
import SchemaBuilder from '@/components/settings/SchemaBuilder.vue';
import ToolTestDialog from '@/components/settings/ToolTestDialog.vue';
import {useMainStore} from '@/stores';
import {Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle} from '@/components/ui/dialog';
import {Tabs, TabsContent, TabsList, TabsTrigger} from '@/components/ui/tabs';
import {Input} from '@/components/ui/input';
import {Textarea} from '@/components/ui/textarea';
import {Label} from '@/components/ui/label';
import {Switch} from '@/components/ui/switch';

const {$customFetch} = useNuxtApp();
const store = useMainStore();

interface ToolFunction {
	id?: string;
	name: string;
	description?: string;
	input_schema: any;
	entrypoint?: string;
	sort_order?: number;
}

interface Tool {
	id: string;
	name: string;
	display_name?: string;
	description?: string;
	icon?: string;
	source_kind: string;
	source_config: any;
	/** @deprecated Use functions array */
	input_schema: any;
	functions: ToolFunction[];
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
const activeFunctionIdx = ref(0);
const activeTab = ref('general');

const isBuiltinTool = computed(() => toolForm.source_kind === 'BUILTIN');

function addfunction() {
	toolForm.functions.push({
		name: '',
		description: '',
		input_schema_json: JSON.stringify({type: 'object', properties: {}}, null, 2),
		entrypoint: '',
	});
	activeFunctionIdx.value = toolForm.functions.length - 1;
}

function removeFunction(idx: number) {
	if (toolForm.functions.length <= 1) return;
	toolForm.functions.splice(idx, 1);
	if (activeFunctionIdx.value >= toolForm.functions.length) {
		activeFunctionIdx.value = Math.max(0, toolForm.functions.length - 1);
	}
}

const sourceKinds = [
	{value: 'BUILTIN', label: 'Builtin', icon: Sparkles, disabled: false},
	{value: 'HTTP', label: 'HTTP', icon: Globe, disabled: false},
	{value: 'WASM', label: 'WASM', icon: Code, disabled: true},
	{value: 'MCP', label: 'MCP', icon: Server, disabled: false},
];

const builtinToolTemplates = [
	{
		name: 'websearch',
		display_name: 'Web Search',
		description: 'Search web using Exa or Tavily for current and accurate information',
		source_kind: 'BUILTIN',
		icon: Globe,
		source_config: {builtin_id: 'websearch'},
		functions: [
			{
				name: 'websearch',
				description: 'Search web for current information',
				input_schema: {
					type: 'object',
					properties: {
						query: {type: 'string', description: 'Search query'},
						num_results: {type: 'integer', description: 'Number of results (1-20)', default: 10},
					},
					required: ['query'],
				},
			},
			{
				name: 'crawl',
				description: 'Crawl a single website content',
				input_schema: {
					type: 'object',
					properties: {
						url: {type: 'string', description: 'URL to crawl'},
					},
					required: ['url'],
				},
			},
		],
		settings_schema: {
			type: 'object',
			required: ['api_key', 'provider'],
			properties: {
				api_key: {type: 'string', title: 'API Key', secret: true, description: 'Get your API key from specified provider'},
				provider: {
					type: 'string',
					title: 'Provider',
					enum: ['exa', 'tavily'],
					description: 'Web search provider to use',
				},
			},
		},
	},
	{
		name: 'imagegen',
		display_name: 'Image Generation',
		description: 'Generate and edit images using OpenAI, Replicate, or Google APIs',
		source_kind: 'BUILTIN',
		icon: Sparkles,
		source_config: {builtin_id: 'imagegen'},
		functions: [
			{
				name: 'generate',
				description: 'Generate an image from a text prompt',
				input_schema: {
					type: 'object',
					properties: {
						prompt: {type: 'string', description: 'The text prompt describing the image to generate'},
						size: {
							type: 'string',
							description: 'Image size',
							enum: ['1024x1024', '1792x1024', '1024x1792', '512x512', '256x256'],
							default: '1024x1024',
						},
						quality: {
							type: 'string',
							description: 'Image quality',
							enum: ['standard', 'hd'],
							default: 'standard',
						},
					},
					required: ['prompt'],
				},
			},
			{
				name: 'edit',
				description: 'Edit an existing image using a text prompt',
				input_schema: {
					type: 'object',
					properties: {
						image_url: {type: 'string', description: 'URL of the image to edit'},
						prompt: {type: 'string', description: 'The text prompt describing the desired edit'},
					},
					required: ['image_url', 'prompt'],
				},
			},
		],
		settings_schema: {
			type: 'object',
			required: ['api_key', 'provider'],
			properties: {
				api_key: {type: 'string', title: 'API Key', secret: true, description: 'API key for the selected provider'},
				provider: {
					type: 'string',
					title: 'Provider',
					enum: ['openai', 'replicate', 'google'],
					description: 'Image generation provider to use',
				},
				model: {
					type: 'string',
					title: 'Model',
					description: 'Model to use (optional, defaults: dall-e-3, flux-schnell, imagen-3)',
				},
			},
		},
	},
];

const displayTools = computed(() => {
	const result: any[] = [...tools.value];

	for (const template of builtinToolTemplates) {
		const exists = tools.value.some(t => t.name === template.name);
		if (!exists) {
			result.push({
				...template,
				id: `template_${template.name}`,
				input_schema: template.functions[0]?.input_schema || {},
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
	functions: [] as {id?: string; name: string; description: string; input_schema_json: string; entrypoint: string}[],
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
	builtin_id: 'websearch',
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

		const result = await $customFetch<{blob_id: string; size_bytes: number}>('/api/v1/admin/tools/wasm/upload', {
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
		const result = await $customFetch('/api/v1/admin/tools');
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
		functions: [
			{
				name: 'default',
				description: '',
				input_schema_json: JSON.stringify({type: 'object', properties: {}}, null, 2),
				entrypoint: '',
			},
		],
		settings_schema_json: '',
		is_enabled: true,
		is_public: false,
	});
	Object.assign(httpConfig, {method: 'GET', url: '', headers_json: '{}', body_template: ''});
	activeFunctionIdx.value = 0;
	activeTab.value = 'general';
	dialogOpen.value = true;
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
			is_public: false,
		};

		const created = (await $customFetch('/api/v1/admin/tools', {method: 'POST', body})) as Tool;
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

function openEditDialog(tool: Tool) {
	editingTool.value = tool;

	const funcs =
		tool.functions?.length > 0
			? tool.functions.map(f => ({
					id: f.id,
					name: f.name,
					description: f.description || '',
					input_schema_json: JSON.stringify(f.input_schema, null, 2),
					entrypoint: f.entrypoint || '',
				}))
			: [
					{
						name: tool.name,
						description: tool.description || '',
						input_schema_json: JSON.stringify(tool.input_schema, null, 2),
						entrypoint: '',
					},
				];

	Object.assign(toolForm, {
		name: tool.name,
		display_name: tool.display_name || '',
		description: tool.description || '',
		source_kind: tool.source_kind,
		functions: funcs,
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

	activeFunctionIdx.value = 0;
	activeTab.value = 'general';
	dialogOpen.value = true;
}

async function openSettingsDialog(tool: Tool) {
	settingsTool.value = tool;
	try {
		const settings = (await $customFetch(`/api/v1/admin/tools/${tool.id}/settings`)) as Record<string, string>;
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
			source_config = {builtin_id: displayTools.value.find(t => t.name === toolForm.name)?.source_config.builtin_id || ''};
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

		const functions = toolForm.functions.map((f, idx) => ({
			id: f.id,
			name: f.name || toolForm.name,
			description: f.description || null,
			input_schema: JSON.parse(f.input_schema_json || '{}'),
			entrypoint: f.entrypoint || null,
		}));

		const body = {
			name: toolForm.name,
			display_name: toolForm.display_name || null,
			description: toolForm.description || null,
			source_kind: toolForm.source_kind,
			source_config,
			functions,
			settings_schema: toolForm.settings_schema_json ? JSON.parse(toolForm.settings_schema_json) : null,
			is_enabled: toolForm.is_enabled,
			is_public: toolForm.is_public,
		};

		if (editingTool.value) {
			await $customFetch(`/api/v1/admin/tools/${editingTool.value.id}`, {method: 'PUT', body});
		} else {
			await $customFetch('/api/v1/admin/tools', {method: 'POST', body});
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
		await $customFetch(`/api/v1/admin/tools/${editingTool.value.id}`, {method: 'DELETE'});
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
		await $customFetch(`/api/v1/admin/tools/${tool.id}`, {
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
		await $customFetch(`/api/v1/admin/tools/${settingsTool.value.id}/settings`, {
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
