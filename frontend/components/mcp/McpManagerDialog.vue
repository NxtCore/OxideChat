<template>
	<ShadDialog :open="open" @update:open="$emit('update:open', $event)">
		<ShadDialogContent class="sm:max-w-[560px]">
			<ShadDialogHeader>
				<ShadDialogTitle>{{ store.getTranslation('mcp.manage_title') }}</ShadDialogTitle>
				<ShadDialogDescription>{{ store.getTranslation('mcp.manage_description') }}</ShadDialogDescription>
			</ShadDialogHeader>

			<div class="space-y-4">
				<div v-if="servers.length === 0" class="py-6 text-center text-sm text-muted-foreground">
					{{ store.getTranslation('mcp.no_servers') }}
				</div>
				<div v-else class="max-h-[38vh] space-y-2 overflow-y-auto pr-1">
					<div v-for="server in servers" :key="server.id" class="rounded-md border border-border p-2">
						<div class="flex items-center gap-2">
							<span class="truncate text-sm font-medium text-foreground">{{ server.name }}</span>
							<ShadBadge variant="secondary" class="text-xs uppercase">{{ server.transport }}</ShadBadge>
							<ShadBadge :variant="healthVariant(server.health_status)" class="text-xs">{{ healthLabel(server.health_status) }}</ShadBadge>
							<span class="ml-auto text-xs text-muted-foreground">{{ server.discovered_tools.length }} {{ store.getTranslation('mcp.discovered_tools') }}</span>
							<ShadSwitch :model-value="server.is_enabled" @update:model-value="toggleEnabled(server, $event)" />
							<ShadButton size="icon" variant="ghost" class="h-8 w-8" :title="store.getTranslation('mcp.discover')" :disabled="busyId === server.id" @click="discover(server)">
								<RefreshCw class="h-4 w-4" :class="busyId === server.id ? 'animate-spin' : ''" />
							</ShadButton>
							<ShadButton size="icon" variant="ghost" class="h-8 w-8" :title="store.getTranslation('mcp.edit')" @click="startEdit(server)">
								<Pencil class="h-4 w-4" />
							</ShadButton>
							<ShadButton size="icon" variant="ghost" class="h-8 w-8 text-destructive" :title="store.getTranslation('mcp.delete')" @click="remove(server)">
								<Trash2 class="h-4 w-4" />
							</ShadButton>
						</div>
						<div v-if="server.discovered_tools.length" class="mt-1 flex flex-wrap gap-1 pl-1">
							<ShadBadge v-for="name in server.discovered_tools" :key="name" variant="outline" class="text-[10px]">{{ name }}</ShadBadge>
						</div>
					</div>
				</div>

				<div class="space-y-3 border-t border-border pt-3">
					<ShadLabel class="text-sm font-medium text-foreground">
						{{ editingId ? store.getTranslation('mcp.edit') : store.getTranslation('mcp.add') }}
					</ShadLabel>

					<div class="grid gap-2">
						<ShadInput v-model="form.name" :placeholder="store.getTranslation('mcp.name_placeholder')" class="h-9" />

						<ShadSelect v-model="form.transport">
							<ShadSelectTrigger class="h-9">
								<ShadSelectValue :placeholder="store.getTranslation('mcp.transport')" />
							</ShadSelectTrigger>
							<ShadSelectContent>
								<ShadSelectItem value="http">{{ store.getTranslation('mcp.transport_http') }}</ShadSelectItem>
								<ShadSelectItem v-if="admin && stdioAllowed" value="stdio">{{ store.getTranslation('mcp.transport_stdio') }}</ShadSelectItem>
							</ShadSelectContent>
						</ShadSelect>

						<template v-if="form.transport === 'stdio'">
							<ShadInput v-model="form.command" :placeholder="store.getTranslation('mcp.command')" class="h-9 font-mono text-xs" />
							<ShadTextarea v-model="form.args" :placeholder="store.getTranslation('mcp.args')" rows="2" class="font-mono text-xs" />
						</template>
					<template v-else>
						<ShadInput v-model="form.url" :placeholder="store.getTranslation('mcp.url_placeholder')" class="h-9" />
						<HeaderEditor v-model="form.headers" />
					</template>
					</div>

					<div class="flex items-center gap-2">
						<ShadButton :disabled="!canSave || saving" @click="save">
							<Plus v-if="!editingId" class="mr-1 h-4 w-4" />
							<Check v-else class="mr-1 h-4 w-4" />
							{{ store.getTranslation('mcp.save') }}
						</ShadButton>
						<ShadButton v-if="editingId" variant="ghost" @click="resetForm">{{ store.getTranslation('mcp.cancel') }}</ShadButton>
					</div>
				</div>
			</div>
		</ShadDialogContent>
	</ShadDialog>
</template>

<script setup lang="ts">
import {ref, reactive, computed, watch} from 'vue';
import {Check, Pencil, Trash2, Plus, RefreshCw} from 'lucide-vue-next';
import HeaderEditor from '@/components/settings/HeaderEditor.vue';
import {useMainStore} from '@/stores';
import type {McpServer} from '~/types/chat';

const props = defineProps<{open: boolean; admin?: boolean}>();
const emit = defineEmits<{(e: 'update:open', value: boolean): void; (e: 'changed'): void}>();

const store = useMainStore();

const apiBase = computed(() => (props.admin ? '/api/v1/admin/mcp-servers' : '/api/v1/mcp-servers'));
const stdioAllowed = computed(() => store.base?.allow_server_stdio_mcp ?? false);

const servers = ref<McpServer[]>([]);
const editingId = ref<string | null>(null);
const saving = ref(false);
const busyId = ref<string | null>(null);

const form = reactive({
	name: '',
	transport: 'http',
	url: '',
	headers: '',
	command: '',
	args: '',
});

const canSave = computed(() => {
	if (form.name.trim() === '') return false;
	if (form.transport === 'stdio') return form.command.trim() !== '';
	return form.url.trim() !== '';
});

watch(
	() => props.open,
	isOpen => {
		if (isOpen) {
			load();
			resetForm();
		}
	}
);

async function load() {
	try {
		const {$customFetch} = useNuxtApp();
		const result = await $customFetch(apiBase.value);
		servers.value = (result as McpServer[]) ?? [];
	} catch (e) {
		console.error('Failed to load MCP servers:', e);
	}
}

function notifyError(e: any, fallbackKey: string) {
	const message = e?.data?.errors?.[0]?.message || store.getTranslation(fallbackKey);
	store.toast(message, {type: 'error'});
}

function buildConnectionConfig(): Record<string, any> {
	if (form.transport === 'stdio') {
		const args = form.args
			.split('\n')
			.map(a => a.trim())
			.filter(Boolean);
		return {command: form.command.trim(), args, env: {}};
	}
	let headers: Record<string, string> = {};
	if (form.headers.trim()) {
		try {
			headers = JSON.parse(form.headers);
		} catch {
			headers = {};
		}
	}
	return {url: form.url.trim(), headers};
}

function resetForm() {
	editingId.value = null;
	form.name = '';
	form.transport = 'http';
	form.url = '';
	form.headers = '';
	form.command = '';
	form.args = '';
}

function startEdit(server: McpServer) {
	editingId.value = server.id;
	form.name = server.name;
	form.transport = server.transport;
	if (server.transport === 'stdio') {
		form.command = server.connection_config?.command ?? '';
		form.args = (server.connection_config?.args ?? []).join('\n');
		form.url = '';
		form.headers = '';
	} else {
		form.url = server.connection_config?.url ?? '';
		const rawHeaders = server.connection_config?.headers;
		form.headers = rawHeaders && Object.keys(rawHeaders).length > 0 ? JSON.stringify(rawHeaders, null, 2) : '';
		form.command = '';
		form.args = '';
	}
}

async function save() {
	if (!canSave.value) return;
	saving.value = true;
	const body = {name: form.name.trim(), transport: form.transport, connection_config: buildConnectionConfig()};
	try {
		const {$customFetch} = useNuxtApp();
		if (editingId.value) {
			await $customFetch(`${apiBase.value}/${editingId.value}`, {method: 'PUT', body});
		} else {
			await $customFetch(apiBase.value, {method: 'POST', body});
		}
		await load();
		emit('changed');
		resetForm();
	} catch (e) {
		notifyError(e, editingId.value ? 'mcp.update_failed' : 'mcp.create_failed');
	} finally {
		saving.value = false;
	}
}

async function toggleEnabled(server: McpServer, value: boolean) {
	try {
		const {$customFetch} = useNuxtApp();
		await $customFetch(`${apiBase.value}/${server.id}`, {method: 'PUT', body: {is_enabled: value}});
		await load();
		emit('changed');
	} catch (e) {
		notifyError(e, 'mcp.update_failed');
	}
}

async function parseMcpResponse(res: Response): Promise<any> {
	const contentType = res.headers.get('content-type') ?? '';
	const text = await res.text();
	if (contentType.includes('text/event-stream')) {
		for (const line of text.split('\n')) {
			if (line.startsWith('data:')) {
				const payload = line.slice(5).trim();
				if (payload) {
					try {
						return JSON.parse(payload);
					} catch {}
				}
			}
		}
		throw new Error('No valid JSON-RPC response in SSE stream');
	}
	return JSON.parse(text);
}

async function discoverLocally(server: McpServer): Promise<{name: string; description: string | null; input_schema: any}[]> {
	const url: string = server.connection_config?.url;
	const headers: Record<string, string> = server.connection_config?.headers ?? {};
	if (!url) throw new Error('No URL configured');

	const baseHeaders = {
		'Content-Type': 'application/json',
		Accept: 'application/json, text/event-stream',
		'MCP-Protocol-Version': '2025-06-18',
		...headers,
	};

	const initRes = await fetch(url, {
		method: 'POST',
		headers: baseHeaders,
		body: JSON.stringify({jsonrpc: '2.0', id: 1, method: 'initialize', params: {protocolVersion: '2025-06-18', capabilities: {}, clientInfo: {name: 'OxideChat', version: '0.1.0'}}}),
	});

	const sessionId = initRes.headers.get('mcp-session-id');
	await parseMcpResponse(initRes);

	const callHeaders: Record<string, string> = {...baseHeaders};
	if (sessionId) callHeaders['Mcp-Session-Id'] = sessionId;

	const listRes = await fetch(url, {
		method: 'POST',
		headers: callHeaders,
		body: JSON.stringify({jsonrpc: '2.0', id: 2, method: 'tools/list'}),
	});

	const data = await parseMcpResponse(listRes);
	if (data.error) throw new Error(data.error.message ?? 'tools/list failed');
	return data.result?.tools ?? [];
}

async function discover(server: McpServer) {
	busyId.value = server.id;
	try {
		const {$customFetch} = useNuxtApp();

		if (props.admin) {
			await $customFetch(`${apiBase.value}/${server.id}/discover`, {method: 'POST'});
		} else {
			const tools = await discoverLocally(server);
			await $customFetch(`${apiBase.value}/${server.id}/sync-tools`, {method: 'POST', body: {tools}});
		}

		await load();
		emit('changed');
	} catch (e) {
		notifyError(e, 'mcp.discover_failed');
	} finally {
		busyId.value = null;
	}
}

async function remove(server: McpServer) {
	try {
		const {$customFetch} = useNuxtApp();
		await $customFetch(`${apiBase.value}/${server.id}`, {method: 'DELETE'});
		if (editingId.value === server.id) resetForm();
		await load();
		emit('changed');
	} catch (e) {
		notifyError(e, 'mcp.delete_failed');
	}
}

function healthLabel(status: string | null): string {
	if (status === 'healthy') return store.getTranslation('mcp.health_healthy');
	if (status === 'unhealthy') return store.getTranslation('mcp.health_unhealthy');
	return store.getTranslation('mcp.health_unknown');
}

function healthVariant(status: string | null): 'default' | 'secondary' | 'destructive' {
	if (status === 'healthy') return 'default';
	if (status === 'unhealthy') return 'destructive';
	return 'secondary';
}
</script>
