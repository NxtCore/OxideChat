<template>
	<Dialog v-model:open="open">
		<DialogContent class="sm:max-w-[550px]">
			<DialogHeader>
				<DialogTitle>{{ tool ? store.getTranslation('settings.tools.edit') : store.getTranslation('settings.tools.create') }}</DialogTitle>
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
						<div class="space-y-2">
							<Label class="text-xs">{{ store.getTranslation('settings.tools.headers') }}</Label>
							<HeaderEditor v-model="httpConfig.headers_json" />
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
							<HeaderEditor v-model="mcpConfig.headers_json" />
						</template>
							<Input v-model="mcpConfig.tool_name" :placeholder="store.getTranslation('settings.tools.tool_name_placeholder')" />
						</div>

						<div class="space-y-2 pt-2 border-t border-border">
							<Label for="tool-system-prompt" class="text-xs text-muted-foreground">{{ store.getTranslation('settings.tools.system_prompt') }}</Label>
							<Textarea
								id="tool-system-prompt"
								v-model="toolForm.system_prompt"
								:placeholder="store.getTranslation('settings.tools.system_prompt_placeholder')"
								rows="3"
							/>
							<p class="text-xs text-muted-foreground">{{ store.getTranslation('settings.tools.system_prompt_hint') }}</p>
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
				<ShadButton v-if="tool && !isBuiltinTool" variant="destructive" size="sm" @click="deleteTool" :disabled="saving" class="mr-auto">
					<Trash2 class="h-3 w-3 mr-1" />
					{{ store.getTranslation('common.delete') }}
				</ShadButton>
				<div class="flex gap-2">
					<ShadButton variant="outline" size="sm" @click="open = false">{{ store.getTranslation('common.cancel') }}</ShadButton>
					<ShadButton size="sm" @click="saveTool" :disabled="saving">
						<Loader2 v-if="saving" class="h-3 w-3 animate-spin mr-1" />
						{{ store.getTranslation('common.save') }}
					</ShadButton>
				</div>
			</DialogFooter>
		</DialogContent>
	</Dialog>
</template>

<script setup lang="ts">
import {Globe, Code, Server, Sparkles, Plus, Trash2, Loader2} from 'lucide-vue-next';
import SchemaBuilder from '@/components/settings/SchemaBuilder.vue';
import HeaderEditor from '@/components/settings/HeaderEditor.vue';
import {useMainStore} from '@/stores';
import type {Tool} from '~/types/tools';
import {Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle} from '@/components/ui/dialog';
import {Tabs, TabsContent, TabsList, TabsTrigger} from '@/components/ui/tabs';
import {Input} from '@/components/ui/input';
import {Textarea} from '@/components/ui/textarea';
import {Label} from '@/components/ui/label';

const props = defineProps<{
	tool: Tool | null;
}>();

const emit = defineEmits<{
	saved: [];
}>();

const open = defineModel<boolean>('open', {default: false});

const store = useMainStore();

const saving = ref(false);
const activeFunctionIdx = ref(0);
const activeTab = ref('general');

const isBuiltinTool = computed(() => toolForm.source_kind === 'BUILTIN');

const sourceKinds = [
	{value: 'BUILTIN', label: 'Builtin', icon: Sparkles, disabled: false},
	{value: 'HTTP', label: 'HTTP', icon: Globe, disabled: false},
	{value: 'WASM', label: 'WASM', icon: Code, disabled: true},
	{value: 'MCP', label: 'MCP', icon: Server, disabled: false},
];

function emptyFunction() {
	return {
		name: '',
		description: '',
		input_schema_json: JSON.stringify({type: 'object', properties: {}}, null, 2),
		entrypoint: '',
	};
}

const toolForm = reactive({
	name: '',
	display_name: '',
	description: '',
	source_kind: 'HTTP',
	functions: [] as {id?: string; name: string; description: string; input_schema_json: string; entrypoint: string}[],
	settings_schema_json: '',
	is_enabled: true,
	system_prompt: '',
});

const httpConfig = reactive({
	method: 'GET',
	url: '',
	headers_json: '{}',
	body_template: '',
});

const wasmConfig = reactive({
	blob_id: '',
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

function addfunction() {
	toolForm.functions.push(emptyFunction());
	activeFunctionIdx.value = toolForm.functions.length - 1;
}

function removeFunction(idx: number) {
	if (toolForm.functions.length <= 1) return;
	toolForm.functions.splice(idx, 1);
	if (activeFunctionIdx.value >= toolForm.functions.length) {
		activeFunctionIdx.value = Math.max(0, toolForm.functions.length - 1);
	}
}

function fillCreateForm() {
	Object.assign(toolForm, {
		name: '',
		display_name: '',
		description: '',
		source_kind: 'HTTP',
		functions: [{...emptyFunction(), name: 'default'}],
		settings_schema_json: '',
		is_enabled: true,
		system_prompt: '',
	});
	Object.assign(httpConfig, {method: 'GET', url: '', headers_json: '{}', body_template: ''});
}

function fillEditForm(tool: Tool) {
	const funcs =
		tool.functions?.length > 0
			? tool.functions.map(f => ({
					id: f.id,
					name: f.name,
					description: f.description || '',
					input_schema_json: JSON.stringify(f.input_schema, null, 2),
					entrypoint: f.entrypoint || '',
				}))
			: [{...emptyFunction(), name: tool.name, description: tool.description || '', input_schema_json: JSON.stringify(tool.input_schema, null, 2)}];

	Object.assign(toolForm, {
		name: tool.name,
		display_name: tool.display_name || '',
		description: tool.description || '',
		source_kind: tool.source_kind,
		functions: funcs,
		settings_schema_json: tool.settings_schema ? JSON.stringify(tool.settings_schema, null, 2) : '',
		is_enabled: tool.is_enabled,
		system_prompt: tool.system_prompt || '',
	});

	if (tool.source_kind === 'HTTP' && tool.source_config) {
		Object.assign(httpConfig, {
			method: tool.source_config.method || 'GET',
			url: tool.source_config.url || '',
			headers_json: JSON.stringify(tool.source_config.headers || {}, null, 2),
			body_template: tool.source_config.body_template || '',
		});
	}
	if (tool.source_kind === 'MCP' && tool.source_config) {
		Object.assign(mcpConfig, {
			transport: tool.source_config.transport || 'stdio',
			command: tool.source_config.command || '',
			args: (tool.source_config.args || []).join(', '),
			url: tool.source_config.url || '',
			headers_json: JSON.stringify(tool.source_config.headers || {}, null, 2),
			tool_name: tool.source_config.tool_name || '',
		});
	}
}

watch(open, isOpen => {
	if (!isOpen) return;
	if (props.tool) fillEditForm(props.tool);
	else fillCreateForm();
	activeFunctionIdx.value = 0;
	activeTab.value = 'general';
});

function buildSourceConfig(): any {
	switch (toolForm.source_kind) {
		case 'HTTP':
			return {
				method: httpConfig.method,
				url: httpConfig.url,
				headers: JSON.parse(httpConfig.headers_json || '{}'),
				body_template: httpConfig.body_template || null,
			};
		case 'BUILTIN':
			return {builtin_id: props.tool?.source_config?.builtin_id || toolForm.name};
		case 'WASM':
			return {wasm_blob_id: wasmConfig.blob_id, entrypoint: wasmConfig.entrypoint || 'execute'};
		case 'MCP':
			if (mcpConfig.transport === 'stdio') {
				return {
					transport: 'stdio',
					command: mcpConfig.command,
					args: mcpConfig.args
						.split(',')
						.map(s => s.trim())
						.filter(Boolean),
					tool_name: mcpConfig.tool_name,
				};
			}
			return {
				transport: 'sse',
				url: mcpConfig.url,
				headers: JSON.parse(mcpConfig.headers_json || '{}'),
				tool_name: mcpConfig.tool_name,
			};
		default:
			return {};
	}
}

async function saveTool() {
	saving.value = true;
	try {
		const body = {
			name: toolForm.name,
			display_name: toolForm.display_name || null,
			description: toolForm.description || null,
			source_kind: toolForm.source_kind,
			source_config: buildSourceConfig(),
			functions: toolForm.functions.map(f => ({
				id: f.id,
				name: f.name || toolForm.name,
				description: f.description || null,
				input_schema: JSON.parse(f.input_schema_json || '{}'),
				entrypoint: f.entrypoint || null,
			})),
			settings_schema: toolForm.settings_schema_json ? JSON.parse(toolForm.settings_schema_json) : null,
			is_enabled: toolForm.is_enabled,
			system_prompt: toolForm.system_prompt?.trim() ? toolForm.system_prompt.trim() : null,
		};

		await saveAdminTool(props.tool?.id, body);

		store.toast('Tool saved successfully', {type: 'success'});
		open.value = false;
		emit('saved');
	} catch (e: any) {
		store.toast('Failed to save tool', {type: 'error', description: e.message});
	} finally {
		saving.value = false;
	}
}

async function deleteTool() {
	if (!props.tool) return;
	saving.value = true;
	try {
		await deleteAdminTool(props.tool.id);
		store.toast('Tool deleted', {type: 'success'});
		open.value = false;
		emit('saved');
	} catch (e: any) {
		store.toast('Failed to delete tool', {type: 'error', description: e.message});
	} finally {
		saving.value = false;
	}
}
</script>
