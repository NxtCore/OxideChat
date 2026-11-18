<template>
	<div class="flex flex-1 flex-col items-center justify-center p-8">
		<template v-if="preset">
			<div class="mb-6 flex h-16 w-16 items-center justify-center overflow-hidden rounded-2xl bg-primary/10" data-testid="preset-empty-state">
				<img v-if="preset.icon" :src="preset.icon" alt="" class="h-full w-full object-cover" />
				<Wand2 v-else class="h-8 w-8 text-primary" />
			</div>
			<h1 class="text-2xl font-semibold text-foreground">{{ preset.name }}</h1>
			<ShadSeparator class="my-6 max-w-2xl" />
			<div v-if="preset.description" :class="descriptionClass" data-testid="chat-description" v-html="renderComplete(preset.description)" />
			<p v-else class="mb-8 max-w-2xl text-center text-muted-foreground">
				{{ store.getTranslation('presets.empty_state.on_model', {model: level?.model?.display_name ?? preset.base_model?.display_name ?? preset.key}) }}
			</p>
		</template>
		<template v-else-if="model">
			<div class="mb-6 flex h-16 w-16 items-center justify-center overflow-hidden rounded-2xl bg-muted" data-testid="model-empty-state">
				<img v-if="model.icon" :src="model.icon" alt="" class="h-full w-full object-cover" />
				<div v-else-if="providerIcon?.type === 'svg'" class="h-full w-full p-3 [&>svg]:h-full [&>svg]:w-full" v-html="sanitizeSvg(providerIcon.icon)" />
				<img v-else-if="providerIcon?.type === 'png'" :src="providerIcon.icon" alt="" class="h-full w-full object-contain p-3" />
				<Bot v-else class="h-8 w-8 text-muted-foreground" />
			</div>
			<h1 class="text-2xl font-semibold text-foreground" :class="{'mb-8': !modelDescription}" data-testid="model-empty-state-name">{{ modelName }}</h1>
			<template v-if="modelDescription">
				<ShadSeparator class="my-6 max-w-2xl" />
				<div :class="descriptionClass" data-testid="chat-description" v-html="renderComplete(modelDescription)" />
			</template>
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
import {Bot, Wand2} from 'lucide-vue-next';
import ChatComposer from './ChatComposer.vue';

import {useMainStore} from '~/stores';
import {useChatStore} from '~/stores/chatStore';
import {useModelStore} from '~/stores/modelStore';
import {useIconsStore} from '~/stores/icons';
import {sanitizeSvg} from '~/composables/useMarkdown';

const emit = defineEmits<{
	send: [content: string, parts?: any[]];
}>();

const store = useMainStore();
const chatStore = useChatStore();
const modelStore = useModelStore();

const {renderComplete} = useMarkdown();

const preset = computed(() => modelStore.selectedModel?.preset ?? null);
const level = computed(() => modelStore.selectedModel?.level ?? null);

const model = computed(() => modelStore.selectedModel);

const modelName = computed(() => model.value?.display_name.replace(/^[^:]+:\s+/, '') || model.value?.display_name);

const modelDescription = computed(() => model.value?.description?.trim() || null);

const iconStore = useIconsStore();

const providerIcon = computed(() => (model.value ? iconStore.getProviderIcon(model.value.provider.name, model.value.model_id) : null));

const descriptionClass = 'prose prose-sm dark:prose-invert mb-8 w-full max-w-2xl text-center text-muted-foreground [&_ol]:list-inside [&_ol]:pl-0 [&_ul]:list-inside [&_ul]:pl-0';

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
