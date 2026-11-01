<template>
	<ChatView />
</template>

<script setup lang="ts">
import {useChatStore} from '~/stores/chatStore';

const chatStore = useChatStore();

if (chatStore.activeChat) chatStore.setActiveChat(null);
</script>
