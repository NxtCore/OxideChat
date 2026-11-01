<template>
	<div class="w-full space-y-4">
		<div>
			<h2 class="text-lg font-semibold text-foreground">{{ store.getTranslation('developer.playground.title') }}</h2>
			<p class="text-sm text-muted-foreground">{{ store.getTranslation('developer.playground.description') }}</p>
		</div>

		<div v-if="developer.loaded && !usableKeys.length" class="flex flex-col items-center gap-2 rounded-lg border border-dashed border-border bg-card py-12 text-center">
			<KeyRound class="h-8 w-8 text-muted-foreground opacity-60" />
			<p class="font-medium text-foreground">{{ store.getTranslation('developer.playground.no_keys') }}</p>
			<ShadButton as-child variant="outline" class="mt-2"><NuxtLink to="/developer/keys">{{ store.getTranslation('developer.keys.create') }}</NuxtLink></ShadButton>
		</div>

		<div v-else class="grid gap-4 xl:grid-cols-[minmax(0,1fr)_19rem]">
			<div class="order-2 min-w-0 rounded-lg border border-border bg-card xl:order-1">
				<ShadTabs v-model="tab" class="flex h-full flex-col gap-0">
					<div class="flex items-center justify-between gap-2 border-b border-border p-2">
						<ShadTabsList>
							<ShadTabsTrigger value="chat">{{ store.getTranslation('developer.playground.tab_chat') }}</ShadTabsTrigger>
							<ShadTabsTrigger value="code" data-testid="playground-tab-code">{{ store.getTranslation('developer.playground.tab_code') }}</ShadTabsTrigger>
							<ShadTabsTrigger value="details" data-testid="playground-tab-details">{{ store.getTranslation('developer.playground.tab_details') }}</ShadTabsTrigger>
						</ShadTabsList>
						<ShadButton v-if="chat.messages.value.length" variant="ghost" size="sm" data-testid="playground-clear" @click="chat.clear">
							<Eraser class="mr-1.5 h-3.5 w-3.5" />{{ store.getTranslation('developer.playground.clear') }}
						</ShadButton>
					</div>

					<ShadTabsContent value="chat" class="mt-0 flex flex-col">
						<div ref="scroller" class="h-[50dvh] space-y-4 overflow-y-auto p-4" data-testid="playground-output">
							<p v-if="!chat.messages.value.length" class="flex h-full items-center justify-center px-6 text-center text-sm text-muted-foreground">{{ store.getTranslation('developer.playground.empty') }}</p>
							<div v-for="(message, index) in chat.messages.value" :key="index" :data-testid="`playground-${message.role}`">
								<p class="mb-1 text-xs font-medium text-muted-foreground">{{ store.getTranslation(message.role === 'user' ? 'developer.playground.you' : 'developer.playground.assistant') }}</p>
								<div v-if="message.content" class="whitespace-pre-wrap break-words rounded-md p-3 text-sm text-foreground" :class="message.role === 'user' ? 'bg-accent/50' : 'bg-muted/50'">{{ message.content }}</div>
								<Loader2 v-else-if="!message.error && chat.running.value" class="h-4 w-4 animate-spin text-muted-foreground" />
								<div v-if="message.error" class="flex flex-wrap items-center justify-between gap-2 rounded-md border border-destructive/50 bg-destructive/10 p-3 text-sm text-foreground" role="alert">
									<span class="min-w-0 break-words">{{ message.error }}</span>
									<ShadButton v-if="isLast(index) && !chat.running.value" variant="outline" size="sm" @click="regenerate">
										<RotateCcw class="mr-1.5 h-3.5 w-3.5" />{{ store.getTranslation('developer.playground.retry') }}
									</ShadButton>
								</div>
								<div v-if="message.metrics && (!chat.running.value || !isLast(index))" class="mt-1.5 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs tabular-nums text-muted-foreground" data-testid="playground-metrics">
									<span v-if="message.metrics.stopped">{{ store.getTranslation('developer.playground.stopped') }}</span>
									<span v-if="message.metrics.ttftMs !== null">{{ store.getTranslation('developer.playground.ttft', {value: formatLatency(message.metrics.ttftMs)}) }}</span>
									<span v-if="message.metrics.totalMs !== null">{{ store.getTranslation('developer.playground.total', {value: formatLatency(message.metrics.totalMs)}) }}</span>
									<span v-if="message.metrics.inputTokens !== null">{{ store.getTranslation('developer.playground.tokens', {input: message.metrics.inputTokens, output: message.metrics.outputTokens ?? 0}) }}</span>
									<span v-if="message.metrics.finishReason">{{ store.getTranslation('developer.playground.finish', {value: message.metrics.finishReason}) }}</span>
									<button v-if="message.metrics.requestId" type="button" class="inline-flex items-center gap-1 font-mono hover:text-foreground" :title="store.getTranslation('developer.playground.copy_request_id')" @click="store.copyToClipboard(message.metrics.requestId ?? '')">
										{{ message.metrics.requestId.slice(0, 8) }}<Copy class="h-3 w-3" />
									</button>
									<button v-if="isLast(index) && !message.error && !chat.running.value" type="button" class="inline-flex items-center gap-1 hover:text-foreground" data-testid="playground-regenerate" @click="regenerate">
										<RotateCcw class="h-3 w-3" />{{ store.getTranslation('developer.playground.regenerate') }}
									</button>
								</div>
							</div>
						</div>

						<div class="border-t border-border p-3">
							<ShadTextarea v-model="prompt" rows="3" data-testid="playground-prompt" :aria-label="store.getTranslation('developer.playground.prompt')" :placeholder="store.getTranslation('developer.playground.prompt_placeholder')" @keydown.enter.ctrl.exact.prevent="send" @keydown.enter.meta.exact.prevent="send" />
							<div class="mt-2 flex items-center justify-between gap-2">
								<span class="text-xs text-muted-foreground">{{ missingHint || store.getTranslation('developer.playground.shortcut') }}</span>
								<ShadButton v-if="!chat.running.value" :disabled="!canSend" data-testid="playground-send" @click="send">
									<SendIcon class="mr-2 h-4 w-4" />{{ store.getTranslation('developer.playground.send') }}
								</ShadButton>
								<ShadButton v-else variant="outline" data-testid="playground-stop" @click="chat.stop">
									<Square class="mr-2 h-4 w-4" />{{ store.getTranslation('developer.playground.stop') }}
								</ShadButton>
							</div>
						</div>
					</ShadTabsContent>

					<ShadTabsContent value="code" class="mt-0 p-4">
						<ShadTabs default-value="curl">
							<ShadTabsList>
								<ShadTabsTrigger v-for="language in LANGUAGES" :key="language" :value="language">{{ store.getTranslation(`developer.playground.lang_${language}`) }}</ShadTabsTrigger>
							</ShadTabsList>
							<ShadTabsContent v-for="language in LANGUAGES" :key="language" :value="language">
								<div class="relative rounded-md bg-muted">
									<ShadButton variant="ghost" size="icon" class="absolute right-2 top-2" :aria-label="store.getTranslation('developer.reveal.copy')" @click="store.copyToClipboard(snippets[language])">
										<Copy class="h-4 w-4" />
									</ShadButton>
									<pre class="max-h-[60dvh] overflow-auto p-4 pr-12 font-mono text-xs leading-relaxed text-foreground" :data-testid="`playground-code-${language}`"><code>{{ snippets[language] }}</code></pre>
								</div>
							</ShadTabsContent>
						</ShadTabs>
						<p class="mt-3 text-xs text-muted-foreground">{{ store.getTranslation('developer.playground.code_hint') }}</p>
					</ShadTabsContent>

					<ShadTabsContent value="details" class="mt-0 space-y-4 p-4">
						<p v-if="!chat.lastRequest.value" class="py-10 text-center text-sm text-muted-foreground">{{ store.getTranslation('developer.playground.details_empty') }}</p>
						<template v-else>
							<div>
								<p class="mb-1.5 text-xs font-medium text-muted-foreground">{{ store.getTranslation('developer.playground.request') }}</p>
								<pre class="max-h-80 overflow-auto rounded-md bg-muted p-3 font-mono text-xs text-foreground" data-testid="playground-request">{{ JSON.stringify(chat.lastRequest.value, null, 2) }}</pre>
							</div>
							<div v-if="chat.lastHeaders.value.length">
								<p class="mb-1.5 text-xs font-medium text-muted-foreground">{{ store.getTranslation('developer.playground.response_headers') }}</p>
								<dl class="space-y-1 rounded-md bg-muted p-3 font-mono text-xs">
									<div v-for="[name, value] in chat.lastHeaders.value" :key="name" class="flex flex-wrap gap-x-2">
										<dt class="text-muted-foreground">{{ name }}</dt>
										<dd class="break-all text-foreground">{{ value }}</dd>
									</div>
								</dl>
							</div>
						</template>
					</ShadTabsContent>
				</ShadTabs>
			</div>

			<aside class="order-1 space-y-4 self-start rounded-lg border border-border bg-card p-4 xl:order-2">
				<div class="space-y-1.5">
					<ShadLabel>{{ store.getTranslation('developer.playground.key') }}</ShadLabel>
					<ShadSelect v-model="keyId">
						<ShadSelectTrigger class="w-full" data-testid="playground-key"><ShadSelectValue :placeholder="store.getTranslation('developer.playground.pick_key')" /></ShadSelectTrigger>
						<ShadSelectContent>
							<ShadSelectItem v-for="apiKey in usableKeys" :key="apiKey.id" :value="apiKey.id">{{ apiKey.name }}</ShadSelectItem>
						</ShadSelectContent>
					</ShadSelect>
					<p v-if="selectedKey" class="truncate font-mono text-xs text-muted-foreground">{{ maskedKey(selectedKey) }}</p>
				</div>

				<div v-if="keyId" class="space-y-1.5">
					<div v-if="savedToken" class="flex items-center justify-between gap-2 rounded-md bg-muted/50 px-3 py-2 text-xs text-muted-foreground" data-testid="playground-token-known">
						<span class="flex min-w-0 items-center gap-1.5"><Check class="h-3.5 w-3.5 shrink-0 text-primary" />{{ store.getTranslation('developer.playground.token_known') }}</span>
						<button type="button" class="shrink-0 underline hover:text-foreground" @click="forgetToken">{{ store.getTranslation('developer.playground.forget') }}</button>
					</div>
					<template v-else>
						<ShadLabel for="playground-token">{{ store.getTranslation('developer.playground.token') }}</ShadLabel>
						<ShadInput id="playground-token" v-model="tokenInput" type="password" autocomplete="off" :placeholder="store.getTranslation('developer.playground.token_placeholder')" @change="saveToken" @paste="pasted" />
						<p class="text-xs text-muted-foreground">{{ store.getTranslation('developer.playground.token_hint') }}</p>
					</template>
				</div>

				<div class="space-y-1.5">
					<ShadLabel>{{ store.getTranslation('developer.playground.model') }}</ShadLabel>
					<DeveloperModelCombobox v-model="settings.model" :models="models" :disabled="!models.length" />
				</div>

				<div class="space-y-1.5">
					<ShadLabel for="playground-system">{{ store.getTranslation('developer.playground.system') }}</ShadLabel>
					<ShadTextarea id="playground-system" v-model="settings.system" rows="3" :placeholder="store.getTranslation('developer.playground.system_placeholder')" />
				</div>

				<div class="grid grid-cols-2 gap-3">
					<div class="space-y-1.5">
						<ShadLabel for="playground-temperature">{{ store.getTranslation('developer.playground.temperature') }}</ShadLabel>
						<ShadInput id="playground-temperature" v-model="settings.temperature" type="number" min="0" max="2" step="0.1" :placeholder="store.getTranslation('developer.playground.default')" />
					</div>
					<div class="space-y-1.5">
						<ShadLabel for="playground-max-tokens">{{ store.getTranslation('developer.playground.max_tokens') }}</ShadLabel>
						<ShadInput id="playground-max-tokens" v-model="settings.maxTokens" type="number" min="1" step="1" :placeholder="store.getTranslation('developer.playground.default')" />
					</div>
				</div>

				<div class="flex items-center justify-between gap-2">
					<ShadLabel for="playground-stream">{{ store.getTranslation('developer.playground.stream') }}</ShadLabel>
					<ShadSwitch id="playground-stream" v-model="settings.stream" />
				</div>
			</aside>
		</div>
	</div>
</template>

<script setup lang="ts">
import {Check, Copy, Eraser, KeyRound, Loader2, RotateCcw, Send as SendIcon, Square} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {useDeveloperStore} from '@/stores/developerStore';

const LANGUAGES = ['curl', 'python', 'javascript'] as const;

const store = useMainStore();
const developer = useDeveloperStore();
const route = useRoute();

const tab = ref('chat');
const keyId = ref('');
const tokenInput = ref('');
const prompt = ref('');
const scroller = ref<HTMLElement | null>(null);
const settings = reactive({model: '', system: '', temperature: '' as string | number, maxTokens: '' as string | number, stream: true});
const chat = usePlaygroundChat(settings, () => store.getTranslation('developer.playground.error'));

const usableKeys = computed(() => developer.activeKeys.filter(key => key.scopes.includes('inference:write')));
const selectedKey = computed(() => developer.keys.find(key => key.id === keyId.value));
const models = computed(() => (selectedKey.value ? (developer.teamById(selectedKey.value.team_id)?.models ?? []) : []));
const savedToken = computed(() => developer.sessionTokens[keyId.value] ?? '');
const missingHint = computed(() => {
	if (!keyId.value) return store.getTranslation('developer.playground.pick_key');
	if (!savedToken.value && !tokenInput.value.trim()) return store.getTranslation('developer.playground.token_placeholder');
	return settings.model ? '' : store.getTranslation('developer.playground.pick_model');
});
const canSend = computed(() => !missingHint.value && prompt.value.trim() !== '');
const snippets = computed(() => {
	const draft = prompt.value.trim();
	const body = chat.requestBody.value;
	const messages = draft ? [...body.messages, {role: 'user', content: draft}] : body.messages;
	return gatewaySnippets(import.meta.client ? gatewayBaseUrl() : '', {...body, messages});
});

function isLast(index: number) {
	return index === chat.messages.value.length - 1;
}

function saveToken() {
	const token = tokenInput.value.trim();
	if (!token || !keyId.value) return;
	developer.sessionTokens[keyId.value] = token;
	tokenInput.value = '';
}

function pasted(event: ClipboardEvent) {
	const token = event.clipboardData?.getData('text').trim();
	if (!token || !keyId.value) return;
	event.preventDefault();
	developer.sessionTokens[keyId.value] = token;
}

function forgetToken() {
	delete developer.sessionTokens[keyId.value];
}

function send() {
	saveToken();
	if (!canSend.value || chat.running.value) return;
	const text = prompt.value.trim();
	prompt.value = '';
	chat.send(text, savedToken.value);
}

function regenerate() {
	if (savedToken.value) chat.regenerate(savedToken.value);
}

watch(keyId, () => {
	tokenInput.value = '';
	if (!models.value.includes(settings.model)) settings.model = models.value[0] ?? '';
});

watch(
	() => chat.messages.value.map(message => message.content.length + (message.metrics?.totalMs ?? 0)).join(),
	() => nextTick(() => scroller.value?.scrollTo({top: scroller.value.scrollHeight})),
);

function applyDefaults() {
	const wanted = String(route.query.model ?? '');
	const owner = usableKeys.value.find(key => developer.teamById(key.team_id)?.models.includes(wanted));
	const remembered = usableKeys.value.find(key => developer.sessionTokens[key.id]);
	const linked = usableKeys.value.find(key => key.id === route.query.key);
	keyId.value = linked?.id ?? (wanted ? owner?.id : undefined) ?? remembered?.id ?? usableKeys.value[0]?.id ?? '';
	nextTick(() => {
		if (wanted && models.value.includes(wanted)) settings.model = wanted;
	});
}

onMounted(() => {
	developer
		.load()
		.then(applyDefaults)
		.catch(() => {});
	developer.loadModels().catch(() => {});
});

onBeforeUnmount(chat.stop);
</script>
