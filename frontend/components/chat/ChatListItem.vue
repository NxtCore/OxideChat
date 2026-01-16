<template>
	<ShadButton
		variant="ghost"
		class="flex w-full items-center gap-3 rounded-lg px-3 py-2 text-sm transition-colors"
		:class="active ? 'bg-muted text-foreground' : 'text-muted-foreground hover:bg-muted/50 hover:text-foreground'"
	>
		<Pin v-if="chat.is_pinned" class="h-3 w-3 shrink-0 text-primary" />
		<MessageSquare v-else class="h-4 w-4 shrink-0" />

		<span class="flex-1 truncate text-left">
			{{ chat.title || store.getTranslation('chat.list.new_chat') }}
		</span>

		<span v-if="chat.message_count > 0" class="shrink-0 text-xs opacity-60">
			{{ chat.message_count }}
		</span>

		<ShadTooltip v-if="chat.branched_from_chat_id">
			<ShadTooltipTrigger as-child>
				<NuxtLink :to="`/chats/${chat.branched_from_chat_id}`" class="text-muted-foreground hover:text-primary" @click.stop>
					<GitBranch class="h-3.5 w-3.5" />
				</NuxtLink>
			</ShadTooltipTrigger>
			<ShadTooltipContent side="right">
				<p class="text-xs">Go to source chat</p>
			</ShadTooltipContent>
		</ShadTooltip>
	</ShadButton>
</template>

<script setup lang="ts">
import {MessageSquare, Pin, GitBranch} from 'lucide-vue-next';
import type {Chat} from '~/types/chat';
import {useMainStore} from '~/stores';

const store = useMainStore();

defineProps<{
	chat: Chat;
	active: boolean;
}>();
</script>
