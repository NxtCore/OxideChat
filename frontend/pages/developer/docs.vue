<template>
	<div class="w-full space-y-4">
		<div>
			<h2 class="text-lg font-semibold text-foreground">{{ store.getTranslation('developer.docs.title') }}</h2>
			<p class="text-sm text-muted-foreground">{{ store.getTranslation('developer.docs.description') }}</p>
		</div>

		<ShadTabs default-value="curl" class="w-full">
			<ShadTabsList class="h-auto w-full flex-wrap justify-start">
				<ShadTabsTrigger v-for="example in examples" :key="example.id" :value="example.id">{{ store.getTranslation(example.label) }}</ShadTabsTrigger>
			</ShadTabsList>
			<ShadTabsContent v-for="example in examples" :key="example.id" :value="example.id">
				<div class="relative rounded-lg border border-border bg-card">
					<ShadButton variant="ghost" size="icon" class="absolute right-2 top-2" :aria-label="store.getTranslation('developer.reveal.copy')" @click="store.copyToClipboard(example.code)">
						<Copy class="h-4 w-4" />
					</ShadButton>
					<pre class="overflow-x-auto p-4 pr-12 font-mono text-xs leading-relaxed text-foreground sm:text-sm"><code>{{ example.code }}</code></pre>
				</div>
			</ShadTabsContent>
		</ShadTabs>

		<p class="text-sm text-muted-foreground">{{ store.getTranslation('developer.docs.placeholder_hint') }}</p>
	</div>
</template>

<script setup lang="ts">
import {Copy} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {useDeveloperStore} from '@/stores/developerStore';

const store = useMainStore();
const developer = useDeveloperStore();

const baseUrl = ref('https://your-host/openai/v1');
const model = computed(() => developer.teams.flatMap(team => team.models)[0] ?? 'provider/model');

const examples = computed(() => [
	{
		id: 'curl',
		label: 'developer.docs.curl',
		code: `curl ${baseUrl.value}/chat/completions \\
  -H "Authorization: Bearer YOUR_API_KEY" \\
  -H "Content-Type: application/json" \\
  -d '{
    "model": "${model.value}",
    "messages": [{"role": "user", "content": "Hello!"}]
  }'`,
	},
	{
		id: 'python',
		label: 'developer.docs.python',
		code: `from openai import OpenAI

client = OpenAI(
    base_url="${baseUrl.value}",
    api_key="YOUR_API_KEY",
)

response = client.chat.completions.create(
    model="${model.value}",
    messages=[{"role": "user", "content": "Hello!"}],
)
print(response.choices[0].message.content)`,
	},
	{
		id: 'javascript',
		label: 'developer.docs.javascript',
		code: `import OpenAI from "openai";

const client = new OpenAI({
  baseURL: "${baseUrl.value}",
  apiKey: "YOUR_API_KEY",
});

const response = await client.chat.completions.create({
  model: "${model.value}",
  messages: [{ role: "user", content: "Hello!" }],
});
console.log(response.choices[0].message.content);`,
	},
	{
		id: 'http',
		label: 'developer.docs.http',
		code: `POST ${new URL(baseUrl.value).pathname}/chat/completions HTTP/1.1
Host: ${new URL(baseUrl.value).host}
Authorization: Bearer YOUR_API_KEY
Content-Type: application/json

{
  "model": "${model.value}",
  "messages": [{"role": "user", "content": "Hello!"}]
}`,
	},
]);

onMounted(() => {
	baseUrl.value = gatewayBaseUrl();
	developer.load().catch(() => {});
});
</script>
