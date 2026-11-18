<template>
	<div class="space-y-3" data-testid="arena-round" :data-decided="round.decided">
		<div class="flex items-center gap-2 text-sm text-muted-foreground">
			<Swords class="h-4 w-4 text-primary" />
			<span>{{ store.getTranslation(round.decided ? 'chat.arena.revealed' : 'chat.arena.pick_hint') }}</span>
		</div>
		<div class="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
			<div
				v-for="(candidate, index) in round.candidates"
				:key="index"
				:data-testid="`arena-candidate-${index}`"
				class="flex min-w-0 flex-col gap-3 rounded-xl border bg-card p-3"
				:class="isWinner(candidate) ? 'border-primary ring-2 ring-primary/30' : 'border-border'"
			>
				<div class="flex items-center justify-between gap-2">
					<span class="text-sm font-semibold text-foreground">{{ store.getTranslation('chat.arena.answer', {label: candidate.label}) }}</span>
					<span v-if="candidate.model_name" class="truncate text-xs text-muted-foreground" data-testid="arena-candidate-model">{{ candidate.model_name }}</span>
				</div>
				<MessageItem :message="candidate.message" blind class="min-w-0 flex-1" />
				<ShadButton
					v-if="!round.decided"
					:data-testid="`arena-pick-${index}`"
					size="sm"
					:disabled="!canPick(candidate) || picking"
					@click="pick(candidate)"
				>
					<Check class="h-4 w-4" />
					{{ store.getTranslation('chat.arena.pick') }}
				</ShadButton>
				<span v-else-if="isWinner(candidate)" class="self-start rounded-full bg-primary/10 px-2 py-0.5 text-xs text-primary">{{ store.getTranslation('chat.arena.picked') }}</span>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import {Check, Swords} from 'lucide-vue-next';
import MessageItem from './MessageItem.vue';
import {useChatStore} from '~/stores/chatStore';
import {useMainStore} from '~/stores';
import type {ArenaCandidate, ArenaRoundState} from '~/types/chat';

const props = defineProps<{round: ArenaRoundState}>();

const store = useMainStore();
const chatStore = useChatStore();
const picking = ref(false);
const winnerId = ref<string | null>(null);

function canPick(candidate: ArenaCandidate): boolean {
	return !chatStore.isStreaming && !candidate.message.id.startsWith('streaming-');
}

function isWinner(candidate: ArenaCandidate): boolean {
	return props.round.decided && candidate.message.id === winnerId.value;
}

async function pick(candidate: ArenaCandidate) {
	const chatId = chatStore.activeChat?.id;
	if (!chatId) return;
	picking.value = true;
	try {
		winnerId.value = candidate.message.id;
		const revealed = await chatStore.pickArenaAnswer(chatId, candidate.message.id);
		const winner = revealed?.candidates.find(item => item.message_id === revealed.winner_message_id);
		if (winner?.model_name) store.toast(store.getTranslation('chat.arena.picked_model', {label: winner.label, model: winner.model_name}), {type: 'success'});
	} catch (e: any) {
		store.toast(store.getTranslation('chat.arena.pick_error'), {type: 'error', description: errorMessage(e, '')});
	} finally {
		picking.value = false;
	}
}
</script>
