<template>
	<div class="flex w-full max-w-3xl flex-col gap-2" data-testid="reasoning-block">
		<button type="button" class="flex w-fit items-center gap-2 text-primary transition-opacity hover:opacity-80 select-none" data-testid="reasoning-toggle" @click="open = !open">
			<Brain class="h-3.5 w-3.5 fill-current" />
			<span class="text-[10px] font-bold uppercase tracking-widest">{{ store.getTranslation('chat.message_item.reasoning') }}</span>
			<ChevronDown class="h-3 w-3 transition-transform" :class="open ? 'rotate-180' : ''" />
		</button>
		<Transition name="expand">
			<div v-if="open" class="rounded-xl bg-muted/50 border p-4" data-testid="reasoning-content">
				<div class="prose prose-sm dark:prose-invert max-w-none opacity-80" v-html="html" @click="emit('code-click', $event)" />
			</div>
		</Transition>
	</div>
</template>

<script setup lang="ts">
import {Brain, ChevronDown} from 'lucide-vue-next';
import {useMainStore} from '~/stores';

defineProps<{html: string}>();
const emit = defineEmits<{'code-click': [event: MouseEvent]}>();
const open = defineModel<boolean>('open', {default: false});
const store = useMainStore();
</script>

<style scoped>
.expand-enter-active,
.expand-leave-active {
	transition: all 0.3s ease;
	overflow: hidden;
}

.expand-enter-from,
.expand-leave-to {
	opacity: 0;
	max-height: 0;
	padding-top: 0;
	padding-bottom: 0;
}

.expand-enter-to,
.expand-leave-from {
	max-height: 500px;
}
</style>
