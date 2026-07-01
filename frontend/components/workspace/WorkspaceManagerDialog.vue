<template>
	<ShadDialog :open="open" @update:open="$emit('update:open', $event)">
		<ShadDialogContent class="sm:max-w-[540px]">
			<ShadDialogHeader>
				<ShadDialogTitle>{{ store.getTranslation('workspace.manage_title') }}</ShadDialogTitle>
				<ShadDialogDescription>{{ store.getTranslation('workspace.manage_description') }}</ShadDialogDescription>
			</ShadDialogHeader>

			<div class="space-y-4">
				<div v-if="chatStore.workspaces.length === 0" class="py-6 text-center text-sm text-muted-foreground">
					{{ store.getTranslation('workspace.empty') }}
				</div>
				<div v-else class="max-h-[40vh] space-y-2 overflow-y-auto pr-1">
					<div v-for="ws in chatStore.workspaces" :key="ws.id" class="rounded-md border border-border p-2">
						<div class="flex items-center gap-2">
							<span class="h-3 w-3 shrink-0 rounded-full border border-border" :style="{backgroundColor: ws.color || 'var(--muted)'}" />
							<template v-if="editingId === ws.id">
								<ShadInput v-model="editingName" class="h-8 flex-1" @keyup.enter="saveRename(ws)" />
								<ShadButton size="icon" variant="ghost" class="h-8 w-8" @click="saveRename(ws)"><Check class="h-4 w-4" /></ShadButton>
								<ShadButton size="icon" variant="ghost" class="h-8 w-8" @click="cancelRename"><X class="h-4 w-4" /></ShadButton>
							</template>
							<template v-else>
								<span class="truncate text-sm font-medium text-foreground">{{ ws.name }}</span>
								<ShadBadge v-if="ws.is_default" variant="secondary" class="text-xs">{{ store.getTranslation('workspace.default_badge') }}</ShadBadge>
								<span class="ml-auto text-xs text-muted-foreground">{{ store.getTranslation('workspace.chat_count', {count: ws.chat_count}) }}</span>
								<ShadButton v-if="!ws.is_default" size="icon" variant="ghost" class="h-8 w-8" :title="store.getTranslation('workspace.set_default')" @click="setDefault(ws)">
									<Star class="h-4 w-4" />
								</ShadButton>
								<ShadButton size="icon" variant="ghost" class="h-8 w-8" :title="store.getTranslation('workspace.rename')" @click="startRename(ws)">
									<Pencil class="h-4 w-4" />
								</ShadButton>
								<ShadButton v-if="!ws.is_default" size="icon" variant="ghost" class="h-8 w-8 text-destructive" :title="store.getTranslation('workspace.delete')" @click="requestDelete(ws)">
									<Trash2 class="h-4 w-4" />
								</ShadButton>
							</template>
						</div>
						<div v-if="editingId === ws.id" class="mt-2 flex items-center gap-1 pl-5">
							<button
								type="button"
								class="flex h-6 w-6 items-center justify-center rounded-full border-2 text-muted-foreground"
								:class="!editingColor ? 'border-foreground' : 'border-transparent'"
								:title="store.getTranslation('workspace.color_none')"
								@click="editingColor = null"
							>
								<Ban class="h-3 w-3" />
							</button>
							<button
								v-for="c in COLORS"
								:key="c"
								type="button"
								class="h-6 w-6 rounded-full border-2"
								:style="{backgroundColor: c}"
								:class="editingColor === c ? 'border-foreground' : 'border-transparent'"
								@click="editingColor = c"
							/>
						</div>
					</div>
				</div>

				<div class="space-y-2 border-t border-border pt-3">
					<ShadLabel class="text-sm text-foreground">{{ store.getTranslation('workspace.create') }}</ShadLabel>
					<div class="flex items-center gap-2">
						<ShadInput v-model="newName" :placeholder="store.getTranslation('workspace.name_placeholder')" class="h-9" @keyup.enter="create" />
						<ShadButton :disabled="!newName.trim() || creating" @click="create">
							<Plus class="mr-1 h-4 w-4" />
							{{ store.getTranslation('workspace.create') }}
						</ShadButton>
					</div>
					<div class="flex items-center gap-1">
						<span class="mr-1 text-xs text-muted-foreground">{{ store.getTranslation('workspace.color') }}</span>
						<button
							type="button"
							class="flex h-6 w-6 items-center justify-center rounded-full border-2 text-muted-foreground"
							:class="!newColor ? 'border-foreground' : 'border-transparent'"
							:title="store.getTranslation('workspace.color_none')"
							@click="newColor = null"
						>
							<Ban class="h-3 w-3" />
						</button>
						<button
							v-for="c in COLORS"
							:key="c"
							type="button"
							class="h-6 w-6 rounded-full border-2"
							:style="{backgroundColor: c}"
							:class="newColor === c ? 'border-foreground' : 'border-transparent'"
							@click="newColor = c"
						/>
					</div>
				</div>
			</div>
		</ShadDialogContent>
	</ShadDialog>

	<DeleteWorkspaceDialog v-model:open="showDelete" :workspace="deleteTarget" />
</template>

<script setup lang="ts">
import {ref} from 'vue';
import {Check, X, Star, Pencil, Trash2, Ban, Plus} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {useChatStore} from '@/stores/chatStore';
import DeleteWorkspaceDialog from './DeleteWorkspaceDialog.vue';
import type {Workspace} from '~/types/chat';

defineProps<{open: boolean}>();
const emit = defineEmits<{(e: 'update:open', value: boolean): void}>();

const store = useMainStore();
const chatStore = useChatStore();

const COLORS = ['#ef4444', '#f97316', '#eab308', '#22c55e', '#14b8a6', '#3b82f6', '#8b5cf6', '#ec4899'];

const newName = ref('');
const newColor = ref<string | null>(null);
const creating = ref(false);

const editingId = ref<string | null>(null);
const editingName = ref('');
const editingColor = ref<string | null>(null);

const showDelete = ref(false);
const deleteTarget = ref<Workspace | null>(null);

async function create() {
	if (!newName.value.trim()) return;
	creating.value = true;
	const created = await chatStore.createWorkspace({name: newName.value.trim(), color: newColor.value ?? undefined});
	creating.value = false;
	if (created) {
		newName.value = '';
		newColor.value = null;
	}
}

function startRename(ws: Workspace) {
	editingId.value = ws.id;
	editingName.value = ws.name;
	editingColor.value = ws.color;
}

function cancelRename() {
	editingId.value = null;
}

async function saveRename(ws: Workspace) {
	const name = editingName.value.trim();
	if (!name) return;
	await chatStore.updateWorkspace(ws.id, {name, color: editingColor.value});
	editingId.value = null;
}

async function setDefault(ws: Workspace) {
	await chatStore.updateWorkspace(ws.id, {is_default: true});
	await chatStore.fetchWorkspaces();
}

function requestDelete(ws: Workspace) {
	deleteTarget.value = ws;
	emit('update:open', false);
	showDelete.value = true;
}
</script>
