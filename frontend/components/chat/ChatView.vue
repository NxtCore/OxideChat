<template>
	<div class="flex h-full flex-col">
		<ChatEmptyState v-if="!chatStore.activeChat" @send="handleSendMessage" />
		<template v-else>
			<MessageList :messages="chatStore.messages" :loading="chatStore.messagesLoading" />
			<ChatInput @send="handleSendMessage" :disabled="chatStore.isStreaming" />
		</template>
	</div>
</template>

<script setup lang="ts">
import {useChatStore} from '~/stores/chatStore';

const chatStore = useChatStore();

async function handleSendMessage(content: string) {
	let chatId = chatStore.activeChat?.id;

	if (!chatId) {
		// Create new chat first
		const chat = await chatStore.createChat({
			workspace_id: chatStore.activeWorkspaceId || undefined,
		});
		if (!chat) return;
		chatId = chat.id;
	}

	// Send message and stream AI response in one call
	await chatStore.sendAndStream(chatId, content);
}

onMounted(async () => {
	await chatStore.init();
});
</script>
