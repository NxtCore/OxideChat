<template>
	<Teleport to="body">
		<div class="fixed inset-0 z-50" @click="emit('close')" @contextmenu.prevent="emit('close')" />
		<div
			ref="menuRef"
			class="fixed z-50 min-w-48 rounded-lg border border-border bg-popover p-1 shadow-lg"
			:style="{left: `${position.x}px`, top: `${position.y}px`}"
		>
			<!-- Pin/Unpin -->
			<ShadButton
				variant="ghost"
				class="flex w-full justify-start items-center gap-2 rounded-md px-3 py-2 text-sm text-popover-foreground hover:bg-accent"
				@click="togglePin"
			>
				<Pin class="h-4 w-4" :class="chat.is_pinned ? 'text-primary' : ''" />
				<span>{{ chat.is_pinned ? 'Unpin' : 'Pin' }}</span>
			</ShadButton>

			<!-- Rename -->
			<ShadButton
				variant="ghost"
				class="flex w-full justify-start items-center gap-2 rounded-md px-3 py-2 text-sm text-popover-foreground hover:bg-accent"
				@click="startRename"
			>
				<Pencil class="h-4 w-4" />
				<span>Rename</span>
			</ShadButton>

			<!-- Move to workspace -->
			<ShadButton
				variant="ghost"
				class="flex w-full justify-start items-center gap-2 rounded-md px-3 py-2 text-sm text-popover-foreground hover:bg-accent"
				@click="showMoveMenu = true"
			>
				<FolderInput class="h-4 w-4" />
				<span>Move to...</span>
			</ShadButton>

			<!-- Archive/Unarchive -->
			<ShadButton
				variant="ghost"
				class="flex w-full justify-start items-center gap-2 rounded-md px-3 py-2 text-sm text-popover-foreground hover:bg-accent"
				@click="toggleArchive"
			>
				<Archive class="h-4 w-4" />
				<span>{{ chat.is_archived ? 'Unarchive' : 'Archive' }}</span>
			</ShadButton>

			<div class="my-1 h-px bg-border" />

			<!-- Export -->
			<ShadButton
				variant="ghost"
				class="flex w-full justify-start items-center gap-2 rounded-md px-3 py-2 text-sm text-popover-foreground hover:bg-accent"
				@click="exportChat"
			>
				<Download class="h-4 w-4" />
				<span>Export</span>
			</ShadButton>

			<div class="my-1 h-px bg-border" />

			<!-- Delete -->
			<ShadButton
				variant="ghost"
				class="flex w-full justify-start items-center gap-2 rounded-md px-3 py-2 text-sm text-destructive hover:bg-destructive/10"
				@click="confirmDelete"
			>
				<Trash2 class="h-4 w-4" />
				<span>Delete</span>
			</ShadButton>
		</div>
	</Teleport>
</template>

<script setup lang="ts">
import {Pin, Pencil, FolderInput, Archive, Download, Trash2} from 'lucide-vue-next';
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
const mainStore = useMainStore();
const menuRef = ref<HTMLElement>();
const showMoveMenu = ref(false);

async function togglePin() {
	await chatStore.updateChat(props.chat.id, {is_pinned: !props.chat.is_pinned});
	emit('close');
}

function startRename() {
	const newTitle = prompt('Enter new title:', props.chat.title || 'New chat');
	if (newTitle !== null) {
		chatStore.updateChat(props.chat.id, {title: newTitle});
	}
	emit('close');
}

async function toggleArchive() {
	await chatStore.updateChat(props.chat.id, {is_archived: !props.chat.is_archived});
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

async function confirmDelete() {
	if (confirm('Are you sure you want to delete this chat? This cannot be undone.')) {
		await chatStore.deleteChat(props.chat.id);
	}
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
