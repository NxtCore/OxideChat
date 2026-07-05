<template>
	<ShadDialog :open="open" @update:open="$emit('update:open', $event)">
		<ShadDialogContent class="sm:max-w-[480px]">
			<ShadDialogHeader>
				<ShadDialogTitle>{{ store.getTranslation('mcp.edit') }}</ShadDialogTitle>
				<ShadDialogDescription>{{ server?.name }}</ShadDialogDescription>
			</ShadDialogHeader>

			<div class="space-y-3 py-2">
				<ShadInput v-model="form.name" :placeholder="store.getTranslation('mcp.name_placeholder')" class="h-9" />

				<ShadSelect v-model="form.transport">
					<ShadSelectTrigger class="h-9">
						<ShadSelectValue :placeholder="store.getTranslation('mcp.transport')" />
					</ShadSelectTrigger>
					<ShadSelectContent>
						<ShadSelectItem value="http">{{ store.getTranslation('mcp.transport_http') }}</ShadSelectItem>
						<ShadSelectItem v-if="stdioAllowed" value="stdio">{{ store.getTranslation('mcp.transport_stdio') }}</ShadSelectItem>
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

			<ShadDialogFooter class="gap-2 pt-2">
				<ShadButton variant="destructive" size="sm" :disabled="saving" class="mr-auto" @click="remove">
					<Trash2 class="h-3.5 w-3.5 mr-1" />
					{{ store.getTranslation('common.delete') }}
				</ShadButton>
				<ShadButton variant="outline" size="sm" @click="$emit('update:open', false)">{{ store.getTranslation('common.cancel') }}</ShadButton>
				<ShadButton size="sm" :disabled="!canSave || saving" @click="save">
					<Loader2 v-if="saving" class="h-3.5 w-3.5 animate-spin mr-1" />
					{{ store.getTranslation('common.save') }}
				</ShadButton>
			</ShadDialogFooter>
		</ShadDialogContent>
	</ShadDialog>
</template>

<script setup lang="ts">
import {ref, reactive, computed, watch} from 'vue';
import {Trash2, Loader2} from 'lucide-vue-next';
import HeaderEditor from '@/components/settings/HeaderEditor.vue';
import {useMainStore} from '@/stores';
import type {McpServer} from '~/types/chat';

const props = defineProps<{open: boolean; server: McpServer | null; admin?: boolean}>();
const emit = defineEmits<{(e: 'update:open', value: boolean): void; (e: 'changed'): void}>();

const store = useMainStore();
const {$customFetch} = useNuxtApp();

const saving = ref(false);
const stdioAllowed = computed(() => store.base?.allow_server_stdio_mcp ?? false);
const apiBase = computed(() => (props.admin ? '/api/v1/admin/mcp-servers' : '/api/v1/mcp-servers'));

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
		if (isOpen && props.server) populateForm(props.server);
	}
);

watch(
	() => props.server,
	server => {
		if (props.open && server) populateForm(server);
	}
);

function populateForm(server: McpServer) {
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

async function save() {
	if (!props.server || !canSave.value) return;
	saving.value = true;
	try {
		await $customFetch(`${apiBase.value}/${props.server.id}`, {
			method: 'PUT',
			body: {name: form.name.trim(), transport: form.transport, connection_config: buildConnectionConfig()},
		});
		emit('changed');
		emit('update:open', false);
	} catch (e: any) {
		const message = e?.data?.errors?.[0]?.message || store.getTranslation('mcp.update_failed');
		store.toast(message, {type: 'error'});
	} finally {
		saving.value = false;
	}
}

async function remove() {
	if (!props.server) return;
	saving.value = true;
	try {
		await $customFetch(`${apiBase.value}/${props.server.id}`, {method: 'DELETE'});
		emit('changed');
		emit('update:open', false);
	} catch (e: any) {
		const message = e?.data?.errors?.[0]?.message || store.getTranslation('mcp.delete_failed');
		store.toast(message, {type: 'error'});
	} finally {
		saving.value = false;
	}
}
</script>
