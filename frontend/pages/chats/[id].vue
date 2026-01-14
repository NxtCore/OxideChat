<template>
	<div class="flex h-full flex-col">
		<MessageList :messages="chatStore.messages" :loading="chatStore.messagesLoading" />
		<ChatComposer @send="handleSendMessage" />
	</div>
</template>

<script setup lang="ts">
import {useChatStore} from '~/stores/chatStore';
import MessageList from '~/components/chat/MessageList.vue';
import ChatComposer from '~/components/chat/ChatComposer.vue';
import {useRoute} from '#app';

const chatStore = useChatStore();
const route = useRoute();

async function handleSendMessage(content: string, parts?: any[]) {
	let chatId = chatStore.activeChat?.id;
	if (!chatId) {
		const chat = await chatStore.createChat({
			workspace_id: chatStore.activeWorkspaceId || undefined,
		});
		if (!chat) return;
		chatId = chat.id;
	}
	await chatStore.sendAndStream(chatId, content, parts);
}

if (!chatStore.activeChat || chatStore.activeChat.id !== route.params.id) {
	await chatStore.fetchChat(route.params.id as string);
}
</script>
