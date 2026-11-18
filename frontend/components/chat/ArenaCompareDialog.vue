<template>
	<ShadDialog :open="open" @update:open="value => emit('update:open', value)">
		<ShadDialogContent class="sm:max-w-5xl">
			<div class="grid gap-4" data-testid="arena-compare-dialog">
				<ShadDialogHeader>
					<ShadDialogTitle>{{ store.getTranslation('chat.arena.compare_title') }}</ShadDialogTitle>
					<ShadDialogDescription>{{ store.getTranslation('chat.arena.compare_description') }}</ShadDialogDescription>
				</ShadDialogHeader>
				<div v-if="!round" class="flex justify-center py-8 text-muted-foreground">
					<Loader2 class="h-5 w-5 animate-spin" />
				</div>
				<div v-else class="grid max-h-[70vh] gap-3 overflow-y-auto md:grid-cols-2 xl:grid-cols-3">
					<div
						v-for="candidate in round.candidates"
						:key="candidate.message_id"
						data-testid="arena-compare-answer"
						class="flex min-w-0 flex-col gap-2 rounded-xl border p-3"
						:class="candidate.message_id === round.winner_message_id ? 'border-primary' : 'border-border'"
					>
						<div class="flex items-center justify-between gap-2">
							<span class="truncate text-sm font-semibold text-foreground" data-testid="arena-compare-model">{{ candidate.model_name ?? candidate.label }}</span>
							<span v-if="candidate.message_id === round.winner_message_id" class="rounded-full bg-primary/10 px-2 py-0.5 text-xs text-primary" data-testid="arena-compare-picked">
								{{ store.getTranslation('chat.arena.picked') }}
							</span>
						</div>
						<div class="prose prose-sm dark:prose-invert max-w-none" v-html="renderComplete(candidate.content)" />
					</div>
				</div>
			</div>
		</ShadDialogContent>
	</ShadDialog>
</template>

<script setup lang="ts">
import {Loader2} from 'lucide-vue-next';
import {useChatStore} from '~/stores/chatStore';
import {useMainStore} from '~/stores';
import {useMarkdown} from '~/composables/useMarkdown';
import type {ArenaRoundResponse} from '~/types/presets';

const props = defineProps<{open: boolean; roundId: string}>();
const emit = defineEmits<{'update:open': [value: boolean]}>();

const store = useMainStore();
const chatStore = useChatStore();
const {renderComplete} = useMarkdown();
const round = ref<ArenaRoundResponse | null>(null);

watch(
	() => props.open,
	async open => {
		const chatId = chatStore.activeChat?.id;
		if (!open || round.value || !chatId) return;
		try {
			round.value = await chatStore.fetchArenaRound(chatId, props.roundId);
		} catch (e: any) {
			store.toast(store.getTranslation('chat.arena.load_error'), {type: 'error', description: errorMessage(e, '')});
			emit('update:open', false);
		}
	},
);
</script>
