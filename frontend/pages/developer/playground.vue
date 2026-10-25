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

		<div v-else class="space-y-4 rounded-lg border border-border bg-card p-4">
			<div class="grid gap-4 sm:grid-cols-2">
				<div class="space-y-1.5">
					<ShadLabel>{{ store.getTranslation('developer.playground.key') }}</ShadLabel>
					<ShadSelect v-model="keyId">
						<ShadSelectTrigger class="w-full"><ShadSelectValue :placeholder="store.getTranslation('developer.playground.pick_key')" /></ShadSelectTrigger>
						<ShadSelectContent>
							<ShadSelectItem v-for="apiKey in usableKeys" :key="apiKey.id" :value="apiKey.id">{{ apiKey.name }} ({{ maskedKey(apiKey) }})</ShadSelectItem>
						</ShadSelectContent>
					</ShadSelect>
				</div>
				<div class="space-y-1.5">
					<ShadLabel>{{ store.getTranslation('developer.playground.model') }}</ShadLabel>
					<DeveloperModelCombobox v-model="model" :models="models" :disabled="!models.length" />
				</div>
			</div>

			<div v-if="keyId" class="space-y-1.5">
				<ShadLabel for="playground-token">{{ store.getTranslation('developer.playground.token') }}</ShadLabel>
				<ShadInput id="playground-token" v-model="token" type="password" autocomplete="off" :placeholder="store.getTranslation('developer.playground.token_placeholder')" />
				<p class="text-xs text-muted-foreground">{{ store.getTranslation('developer.playground.token_hint') }}</p>
			</div>

			<div class="space-y-1.5">
				<ShadLabel for="playground-prompt">{{ store.getTranslation('developer.playground.prompt') }}</ShadLabel>
				<ShadTextarea id="playground-prompt" v-model="prompt" rows="4" />
			</div>

			<div class="flex flex-wrap gap-2">
				<ShadButton v-if="!running" :disabled="!canSend" @click="send">
					<Send class="mr-2 h-4 w-4" />{{ store.getTranslation('developer.playground.send') }}
				</ShadButton>
				<ShadButton v-else variant="outline" @click="stop">
					<Square class="mr-2 h-4 w-4" />{{ store.getTranslation('developer.playground.stop') }}
				</ShadButton>
			</div>

			<div v-if="errorText" class="rounded-md border border-destructive/50 bg-destructive/10 p-3 text-sm text-foreground" role="alert">{{ errorText }}</div>

			<div v-if="output || running" class="space-y-1.5">
				<p class="text-xs font-medium uppercase tracking-wider text-muted-foreground">{{ store.getTranslation('developer.playground.response') }}</p>
				<pre class="max-h-96 overflow-auto whitespace-pre-wrap break-words rounded-md bg-muted p-3 font-sans text-sm text-foreground" data-testid="playground-output">{{ output }}<Loader2 v-if="running && !output" class="inline h-4 w-4 animate-spin" /></pre>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import {KeyRound, Loader2, Send, Square} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {useDeveloperStore} from '@/stores/developerStore';

const store = useMainStore();
const developer = useDeveloperStore();

const keyId = ref('');
const model = ref('');
const token = ref('');
const prompt = ref('');
const output = ref('');
const errorText = ref('');
const running = ref(false);
let controller: AbortController | null = null;

const usableKeys = computed(() => developer.activeKeys.filter(key => key.scopes.includes('inference:write')));
const models = computed(() => {
	const selected = developer.keys.find(key => key.id === keyId.value);
	return selected ? (developer.teamById(selected.team_id)?.models ?? []) : [];
});
const canSend = computed(() => !!keyId.value && !!model.value && token.value.trim() !== '' && prompt.value.trim() !== '');

watch(keyId, (id) => {
	token.value = developer.sessionTokens[id] ?? '';
	model.value = models.value[0] ?? '';
});

function stop() {
	controller?.abort();
}

async function readFailure(response: Response) {
	const body = await response.json().catch(() => null);
	return body?.error?.message ?? body?.message ?? `HTTP ${response.status}`;
}

function appendDelta(line: string) {
	if (!line.startsWith('data:')) return;
	const data = line.slice(5).trim();
	if (!data || data === '[DONE]') return;
	try {
		output.value += JSON.parse(data).choices?.[0]?.delta?.content ?? '';
	} catch {
		return;
	}
}

async function consume(response: Response) {
	const reader = response.body?.getReader();
	if (!reader) return;
	const decoder = new TextDecoder();
	let buffer = '';
	for (;;) {
		const {done, value} = await reader.read();
		if (done) break;
		buffer += decoder.decode(value, {stream: true});
		const lines = buffer.split('\n');
		buffer = lines.pop() ?? '';
		lines.forEach(appendDelta);
	}
	appendDelta(buffer);
}

async function send() {
	controller = new AbortController();
	running.value = true;
	output.value = '';
	errorText.value = '';
	try {
		const response = await fetch(`${gatewayBaseUrl()}/chat/completions`, {
			method: 'POST',
			signal: controller.signal,
			headers: {'Content-Type': 'application/json', Authorization: `Bearer ${token.value.trim()}`},
			body: JSON.stringify({model: model.value, stream: true, messages: [{role: 'user', content: prompt.value}]}),
		});
		if (!response.ok) {
			errorText.value = await readFailure(response);
			return;
		}
		await consume(response);
	} catch (e: any) {
		if (e?.name !== 'AbortError') errorText.value = e?.message ?? store.getTranslation('developer.playground.error');
	} finally {
		running.value = false;
		controller = null;
	}
}

onMounted(() => {
	developer.load().catch(() => {});
});

onBeforeUnmount(stop);
</script>
