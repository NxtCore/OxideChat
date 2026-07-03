<template>
	<Teleport to="body">
		<div class="fixed inset-0 z-50" @click="emit('close')" @contextmenu.prevent="emit('close')" />
		<div
			ref="menuRef"
			class="fixed z-50 min-w-48 rounded-lg border border-border bg-popover p-1 shadow-lg"
			:style="{left: `${position.x}px`, top: `${position.y}px`}"
		>
			<template v-if="showMoveMenu">
				<ShadButton
					variant="ghost"
					class="flex w-full justify-start items-center gap-2 rounded-md px-3 py-2 text-sm text-popover-foreground hover:bg-accent"
					@click="showMoveMenu = false"
				>
					<ChevronLeft class="h-4 w-4" />
					<span>{{ store.getTranslation('chat.context_menu.move_to') }}</span>
				</ShadButton>

				<div class="my-1 h-px bg-border" />

				<div class="max-h-64 overflow-y-auto">
					<ShadButton
						v-for="workspace in chatStore.workspaces"
						:key="workspace.id"
						variant="ghost"
						class="flex w-full justify-start items-center gap-2 rounded-md px-3 py-2 text-sm text-popover-foreground hover:bg-accent"
						:disabled="workspace.id === chat.workspace_id"
						@click="moveToWorkspace(workspace.id)"
					>
						<span class="h-2.5 w-2.5 shrink-0 rounded-full border border-border" :style="{backgroundColor: workspace.color || 'var(--muted)'}" />
						<span class="truncate">{{ workspace.name }}</span>
						<Check v-if="workspace.id === chat.workspace_id" class="ml-auto h-4 w-4 text-primary" />
					</ShadButton>

					<ShadButton
						variant="ghost"
						class="flex w-full justify-start items-center gap-2 rounded-md px-3 py-2 text-sm text-popover-foreground hover:bg-accent"
						:disabled="!chat.workspace_id"
						@click="moveToWorkspace(null)"
					>
						<Ban class="h-4 w-4 text-muted-foreground" />
						<span>{{ store.getTranslation('chat.context_menu.no_workspace') }}</span>
						<Check v-if="!chat.workspace_id" class="ml-auto h-4 w-4 text-primary" />
					</ShadButton>
				</div>
			</template>

			<template v-else>
			<ShadButton
				variant="ghost"
				class="flex w-full justify-start items-center gap-2 rounded-md px-3 py-2 text-sm text-popover-foreground hover:bg-accent"
				@click="togglePin"
			>
				<Pin class="h-4 w-4" :class="chat.is_pinned ? 'text-primary' : ''" />
				<span>{{ chat.is_pinned ? store.getTranslation('chat.context_menu.unpin') : store.getTranslation('chat.context_menu.pin') }}</span>
			</ShadButton>

			<ShadButton
				variant="ghost"
				class="flex w-full justify-start items-center gap-2 rounded-md px-3 py-2 text-sm text-popover-foreground hover:bg-accent"
				@click="openRenameDialog"
			>
				<Pencil class="h-4 w-4" />
				<span>{{ store.getTranslation('chat.context_menu.rename') }}</span>
			</ShadButton>

			<ShadButton
				variant="ghost"
				class="flex w-full justify-start items-center gap-2 rounded-md px-3 py-2 text-sm text-popover-foreground hover:bg-accent"
				@click="showMoveMenu = true"
			>
				<FolderInput class="h-4 w-4" />
				<span>{{ store.getTranslation('chat.context_menu.move_to') }}</span>
			</ShadButton>

			<ShadButton
				variant="ghost"
				class="flex w-full justify-start items-center gap-2 rounded-md px-3 py-2 text-sm text-popover-foreground hover:bg-accent"
				@click="toggleArchive"
			>
				<Archive class="h-4 w-4" />
				<span>{{ chat.is_archived ? store.getTranslation('chat.context_menu.unarchive') : store.getTranslation('chat.context_menu.archive') }}</span>
			</ShadButton>

			<div class="my-1 h-px bg-border" />

			<ShadButton
				variant="ghost"
				class="flex w-full justify-start items-center gap-2 rounded-md px-3 py-2 text-sm text-popover-foreground hover:bg-accent"
				@click="exportChat"
			>
				<Download class="h-4 w-4" />
				<span>{{ store.getTranslation('chat.context_menu.export') }}</span>
			</ShadButton>

			<div class="my-1 h-px bg-border" />

			<ShadButton
				variant="ghost"
				class="flex w-full justify-start items-center gap-2 rounded-md px-3 py-2 text-sm text-destructive hover:bg-destructive/10"
				@click="openDeleteDialog"
			>
				<Trash2 class="h-4 w-4" />
				<span>{{ store.getTranslation('chat.context_menu.delete') }}</span>
			</ShadButton>
			</template>
		</div>

		<ShadDialog v-model:open="showRenameDialog">
			<ShadDialogContent class="sm:max-w-md">
				<ShadDialogHeader>
					<ShadDialogTitle>{{ store.getTranslation('chat.context_menu.rename') }}</ShadDialogTitle>
					<ShadDialogDescription>{{ store.getTranslation('chat.context_menu.rename_prompt') }}</ShadDialogDescription>
				</ShadDialogHeader>
				<div class="py-4">
					<ShadInput v-model="renameValue" :placeholder="store.getTranslation('chat.list.new_chat')" @keydown.enter="submitRename" />
				</div>
				<ShadDialogFooter>
					<ShadButton variant="outline" @click="showRenameDialog = false">
						{{ store.getTranslation('common.cancel') }}
					</ShadButton>
					<ShadButton @click="submitRename">
						{{ store.getTranslation('common.save') }}
					</ShadButton>
				</ShadDialogFooter>
			</ShadDialogContent>
		</ShadDialog>

		<ShadDialog v-model:open="showDeleteDialog">
			<ShadDialogContent class="sm:max-w-md">
				<ShadDialogHeader>
					<ShadDialogTitle>{{ store.getTranslation('chat.context_menu.delete') }}</ShadDialogTitle>
					<ShadDialogDescription>{{ store.getTranslation('chat.context_menu.delete_confirm') }}</ShadDialogDescription>
				</ShadDialogHeader>
				<ShadDialogFooter>
					<ShadButton variant="outline" @click="showDeleteDialog = false">
						{{ store.getTranslation('common.cancel') }}
					</ShadButton>
					<ShadButton variant="destructive" @click="executeDelete">
						{{ store.getTranslation('chat.context_menu.delete') }}
					</ShadButton>
				</ShadDialogFooter>
			</ShadDialogContent>
		</ShadDialog>
	</Teleport>
</template>

<script setup lang="ts">
import {Pin, Pencil, FolderInput, Archive, Download, Trash2, ChevronLeft, Check, Ban} from 'lucide-vue-next';
import type {Chat} from '~/types/chat';
import {useChatStore} from '~/stores/chatStore';
import {useMainStore} from '~/stores';

const props = defineProps<{
	chat: Chat;
	position: {x: number; y: number};
}>();

const emit = defineEmits<{
	close: [];
}>();

const chatStore = useChatStore();
const store = useMainStore();
const menuRef = ref<HTMLElement>();
const showMoveMenu = ref(false);
const showRenameDialog = ref(false);
const showDeleteDialog = ref(false);
const renameValue = ref('');

async function togglePin() {
	await chatStore.updateChat(props.chat.id, {is_pinned: !props.chat.is_pinned});
	emit('close');
}

function openRenameDialog() {
	renameValue.value = props.chat.title || store.getTranslation('chat.list.new_chat');
	showRenameDialog.value = true;
}

async function submitRename() {
	if (renameValue.value.trim()) {
		await chatStore.updateChat(props.chat.id, {title: renameValue.value.trim()});
	}
	showRenameDialog.value = false;
	emit('close');
}

async function toggleArchive() {
	await chatStore.updateChat(props.chat.id, {is_archived: !props.chat.is_archived});
	emit('close');
}

async function moveToWorkspace(workspaceId: string | null) {
	if (workspaceId === (props.chat.workspace_id ?? null)) {
		emit('close');
		return;
	}
	await chatStore.updateChat(props.chat.id, {workspace_id: workspaceId});
	await chatStore.fetchChats({workspace_id: chatStore.activeWorkspaceId || undefined});
	chatStore.fetchWorkspaces();
	emit('close');
}

function exportChat() {
	// Export as JSON
	const data = {
		chat: props.chat,
		messages: chatStore.messages,
		exported_at: new Date().toISOString(),
	};
	const blob = new Blob([JSON.stringify(data, null, 2)], {type: 'application/json'});
	const url = URL.createObjectURL(blob);
	const a = document.createElement('a');
	a.href = url;
	a.download = `chat-${props.chat.id}.json`;
	a.click();
	URL.revokeObjectURL(url);
	emit('close');
}

function openDeleteDialog() {
	showDeleteDialog.value = true;
}

async function executeDelete() {
	await chatStore.deleteChat(props.chat.id);
	showDeleteDialog.value = false;
	emit('close');
}

// Adjust position to keep menu in viewport
onMounted(() => {
	const menu = menuRef.value;
	if (!menu) return;

	const rect = menu.getBoundingClientRect();
	if (rect.right > window.innerWidth) {
		menu.style.left = `${window.innerWidth - rect.width - 8}px`;
	}
	if (rect.bottom > window.innerHeight) {
		menu.style.top = `${window.innerHeight - rect.height - 8}px`;
	}
});
</script>
