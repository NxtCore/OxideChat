<template>
	<div class="flex h-full flex-col">
		<!-- Main chat area placeholder -->
		<div class="flex flex-1 items-center justify-center">
			<div class="text-center">
				<div class="mx-auto mb-6 flex h-20 w-20 items-center justify-center rounded-full bg-gradient-to-br from-violet-400 to-fuchsia-500">
					<MessageSquare class="h-10 w-10 text-white" />
				</div>
				<h1 class="text-2xl font-bold text-white">Welcome to OxideChat</h1>
				<p class="mt-2 text-zinc-400">Select a chat from the sidebar or start a new conversation</p>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import {MessageSquare} from 'lucide-vue-next';
import {useMainStore} from '@/stores';

const store = useMainStore();

// Page is protected by auth middleware
</script>
