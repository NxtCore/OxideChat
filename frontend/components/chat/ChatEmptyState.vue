<template>
	<div class="flex flex-1 flex-col items-center justify-center p-8">
		<template v-if="preset">
			<div class="mb-6 flex h-16 w-16 items-center justify-center overflow-hidden rounded-2xl bg-primary/10" data-testid="preset-empty-state">
				<img v-if="preset.icon" :src="preset.icon" alt="" class="h-full w-full object-cover" />
				<Wand2 v-else class="h-8 w-8 text-primary" />
			</div>
			<h1 class="mb-2 text-2xl font-semibold text-foreground">{{ preset.name }}</h1>
			<p class="mb-8 max-w-md text-center text-muted-foreground">
				{{ preset.description || store.getTranslation('presets.empty_state.on_model', {model: preset.base_model?.display_name ?? preset.key}) }}
			</p>
		</template>
		<template v-else>
			<img :src="store.appIcon" alt="" class="h-20 w-20 mb-8 object-contain" />
			<h1 class="mb-2 text-2xl font-semibold text-foreground">{{ greeting }}, {{ username }}</h1>
			<p class="mb-8 max-w-md text-center text-muted-foreground">
				{{ description }}
			</p>
		</template>
		<div class="mb-4 w-full max-w-2xl">
			<ChatComposer @send="onSend" />
		</div>
		<div v-if="preset && preset.starters.length > 0" class="grid w-full max-w-2xl grid-cols-1 gap-2 sm:grid-cols-2">
			<button
				v-for="starter in preset.starters"
				:key="starter"
				data-testid="preset-starter"
				class="rounded-xl border border-border bg-card px-4 py-3 text-left text-sm text-foreground transition-colors hover:bg-accent disabled:opacity-50"
				:disabled="chatStore.isStreaming"
				@click="onSend(starter, [{type: 'text', text: starter}])"
			>
				{{ starter }}
			</button>
		</div>
	</div>
</template>

<script setup lang="ts">
import {Wand2} from 'lucide-vue-next';
import ChatComposer from './ChatComposer.vue';

import {useMainStore} from '~/stores';
import {useChatStore} from '~/stores/chatStore';
import {useModelStore} from '~/stores/modelStore';

const emit = defineEmits<{
	send: [content: string, parts?: any[]];
}>();

const store = useMainStore();
const chatStore = useChatStore();
const modelStore = useModelStore();

const preset = computed(() => modelStore.selectedModel?.preset ?? null);

const username = computed(() => store.auth.user?.username || store.getTranslation('chat.empty_state.username_default'));

const greeting = computed(() => {
	const hour = new Date().getHours();
	if (hour < 12) return store.getTranslation('chat.empty_state.greeting_morning');
	if (hour < 18) return store.getTranslation('chat.empty_state.greeting_afternoon');
	return store.getTranslation('chat.empty_state.greeting_evening');
});

const description = computed(() => {
	const placeholders = [
		store.getTranslation('chat.empty_state.desc_1'),
		store.getTranslation('chat.empty_state.desc_2'),
		store.getTranslation('chat.empty_state.desc_3'),
		store.getTranslation('chat.empty_state.desc_4'),
		store.getTranslation('chat.empty_state.desc_5'),
	];
	return placeholders[Math.floor(Math.random() * placeholders.length)];
});

function onSend(content: string, parts?: any[]) {
	if (!content && (!parts || parts.length === 0)) return;
	emit('send', content, parts);
}
</script>
