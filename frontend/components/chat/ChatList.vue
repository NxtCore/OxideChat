<template>
	<div class="flex flex-col h-full">
		<ShadButton
			class="mx-2 mb-4 flex items-center gap-2 rounded-lg bg-primary px-4 py-3 text-sm font-bold text-primary-foreground transition-colors hover:bg-primary/90"
			@click="createNewChat"
		>
			<Plus class="h-4 w-4" />
			<span>{{ store.getTranslation('chat.list.new_chat') }}</span>
		</ShadButton>
		<div class="flex-1 space-y-1 overflow-y-auto px-2">
			<template v-if="chatStore.pinnedChats.length > 0">
				<div class="mb-2 px-2 text-xs font-medium text-muted-foreground">{{ store.getTranslation('chat.list.pinned') }}</div>
				<ChatListItem
					v-for="chat in chatStore.pinnedChats"
					:key="chat.id"
					:chat="chat"
					:active="chat.id === chatStore.activeChat?.id"
					@click="selectChat(chat)"
					@contextmenu="showContextMenu($event, chat)"
				/>
			</template>

			<template v-for="group in groupedChats" :key="group.label">
				<div class="mb-2 mt-4 px-2 text-xs font-medium text-muted-foreground">{{ store.getTranslation('chat.list.' + group.label.replace(/\s+/g, '_').toLowerCase()) }}</div>
				<ChatListItem
					v-for="chat in group.chats"
					:key="chat.id"
					:chat="chat"
					:active="chat.id === chatStore.activeChat?.id"
					@click="selectChat(chat)"
					@contextmenu="showContextMenu($event, chat)"
				/>
			</template>
		</div>

		<ChatContextMenu v-if="contextMenuChat" :chat="contextMenuChat" :position="contextMenuPosition" @close="contextMenuChat = null" />
	</div>
</template>

<script setup lang="ts">
import {Plus} from 'lucide-vue-next';
import ChatListItem from './ChatListItem.vue';
import ChatContextMenu from './ChatContextMenu.vue';
import type {Chat} from '~/types/chat';
import {useChatStore} from '~/stores/chatStore';
import {useMainStore} from '~/stores';
import {computed, ref} from 'vue';
import {useRouter} from 'vue-router';

const router = useRouter();

const chatStore = useChatStore();
const store = useMainStore();

const contextMenuChat = ref<Chat | null>(null);
const contextMenuPosition = ref({x: 0, y: 0});

interface ChatGroup {
	label: string;
	chats: Chat[];
}

const groupedChats = computed((): ChatGroup[] => {
	const now = new Date();
	const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
	const yesterday = new Date(today.getTime() - 86400000);
	const weekAgo = new Date(today.getTime() - 7 * 86400000);
	const monthAgo = new Date(today.getTime() - 30 * 86400000);

	const groups: Record<string, Chat[]> = {
		Today: [],
		Yesterday: [],
		'Past 7 days': [],
		'Past 30 days': [],
		Older: [],
	};

	for (const chat of chatStore.recentChats) {
		const date = new Date(chat.updated_at);
		if (date >= today) {
			groups['Today']!.push(chat);
		} else if (date >= yesterday) {
			groups['Yesterday']!.push(chat);
		} else if (date >= weekAgo) {
			groups['Past 7 days']!.push(chat);
		} else if (date >= monthAgo) {
			groups['Past 30 days']!.push(chat);
		} else {
			groups['Older']!.push(chat);
		}
	}

	return Object.entries(groups)
		.filter(([_, chats]) => chats.length > 0)
		.map(([label, chats]) => ({label, chats}));
});

function selectChat(chat: Chat) {
	router.push(`/chats/${chat.id}`);
}

async function createNewChat() {
	router.push('/');
	chatStore.setActiveChat(null);
}

function showContextMenu(event: MouseEvent, chat: Chat) {
	event.preventDefault();
	contextMenuChat.value = chat;
	contextMenuPosition.value = {x: event.clientX, y: event.clientY};
}
</script>
