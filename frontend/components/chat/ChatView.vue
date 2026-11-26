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
const router = useRouter();

async function handleSendMessage(content: string, parts?: any[]) {
	let chatId = chatStore.activeChat?.id;
	const isNew = !chatId;

	if (!chatId) {
		const chat = await chatStore.createChat({
			workspace_id: workspaceStore.activeWorkspaceId || undefined,
		});
		if (!chat) return;
		chatId = chat.id;
	}

	const refused = await sendAndStream(chatId, content, parts);
	if (isNew && refused && (await chatStore.discardIfEmpty(chatId))) {
		await router.push('/');
		chatStore.stageDraft(content, parts);
	}
}
</script>
