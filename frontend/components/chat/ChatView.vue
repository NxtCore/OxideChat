<template>
	<div class="flex h-full flex-col">
		<ChatEmptyState @send="handleSendMessage" />
	</div>
</template>

<script setup lang="ts">
import {useChatStore} from '~/stores/chatStore';
import {useWorkspaceStore} from '~/stores/workspaceStore';
import ChatEmptyState from './ChatEmptyState.vue';

const chatStore = useChatStore();
const workspaceStore = useWorkspaceStore();
const {sendAndStream} = useChatStream();

async function handleSendMessage(content: string, parts?: any[]) {
	let chatId = chatStore.activeChat?.id;

	if (!chatId) {
		const chat = await chatStore.createChat({
			workspace_id: workspaceStore.activeWorkspaceId || undefined,
		});
		if (!chat) return;
		chatId = chat.id;
	}

	await sendAndStream(chatId, content, parts);
}
</script>
