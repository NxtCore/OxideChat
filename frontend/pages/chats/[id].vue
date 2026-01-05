<template>
	<div class="flex h-full flex-col">
		<MessageList :messages="chatStore.messages" :loading="chatStore.messagesLoading" />
		<ChatComposer @send="handleSendMessage" />
	</div>
</template>

<script setup lang="ts">
import {useChatStore} from '~/stores/chatStore';
import ChatInput from '~/components/chat/ChatInput.vue';
import MessageList from '~/components/chat/MessageList.vue';
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

	// Send message and stream AI response in one call
	await chatStore.sendAndStream(chatId, content);
}

onMounted(async () => {
	await chatStore.init();
});
</script>
