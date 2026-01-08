<template>
	<div class="flex h-full flex-col">
		<ChatEmptyState @send="handleSendMessage" />
	</div>
</template>

<script setup lang="ts">
import {useChatStore} from '~/stores/chatStore';
import ChatEmptyState from './ChatEmptyState.vue';

const chatStore = useChatStore();

async function handleSendMessage(content: string) {
	let chatId = chatStore.activeChat?.id;

	if (!chatId) {
		const chat = await chatStore.createChat({
			workspace_id: chatStore.activeWorkspaceId || undefined,
		});
		if (!chat) return;
		chatId = chat.id;
	}

	await chatStore.sendAndStream(chatId, content);
}
</script>
