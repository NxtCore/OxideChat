<template>
	<div v-if="siblingCount > 1" class="flex items-center gap-1 text-xs text-muted-foreground select-none">
		<ShadButton variant="ghost" size="icon" class="h-5 w-5 text-muted-foreground hover:text-foreground" :disabled="currentIndex <= 1" @click="navigate('prev')">
			<ChevronLeft class="h-3 w-3" />
		</ShadButton>
		<span class="min-w-[3rem] text-center text-xs tabular-nums"> {{ currentIndex }} / {{ siblingCount }} </span>
		<ShadButton
			variant="ghost"
			size="icon"
			class="h-5 w-5 text-muted-foreground hover:text-foreground"
			:disabled="currentIndex >= siblingCount"
			@click="navigate('next')"
		>
			<ChevronRight class="h-3 w-3" />
		</ShadButton>
	</div>
</template>

<script setup lang="ts">
import {ChevronLeft, ChevronRight} from 'lucide-vue-next';

const props = defineProps<{
	messageId: string;
	currentIndex: number;
	siblingCount: number;
}>();

const emit = defineEmits<{
	navigate: [direction: 'prev' | 'next'];
}>();

function navigate(direction: 'prev' | 'next') {
	emit('navigate', direction);
}
</script>
