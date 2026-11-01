<template>
	<div class="space-y-5">
		<div>
			<h3 class="text-base font-semibold text-foreground mb-1">
				{{ store.getTranslation('settings.models.editor.core_prompt') }}
			</h3>
			<p class="text-xs text-muted-foreground">
				{{ store.getTranslation('settings.models.editor.core_prompt_desc') }}
			</p>
		</div>

		<div class="space-y-1.5">
			<div class="flex items-center justify-between">
				<ShadLabel for="system-prompt">
					{{ store.getTranslation('settings.models.editor.system_prompt') }}
				</ShadLabel>
				<span class="text-xs text-muted-foreground">
					{{ form.system_prompt?.length || 0 }} {{ store.getTranslation('settings.models.editor.characters') }}
				</span>
			</div>
			<ShadTextarea
				id="system-prompt"
				ref="systemPromptTextareaRef"
				v-model="form.system_prompt"
				:placeholder="store.getTranslation('settings.models.editor.system_prompt_placeholder')"
				rows="12"
				class="font-mono text-sm"
			/>
			<p class="text-xs text-muted-foreground">
				{{ store.getTranslation('settings.models.editor.system_prompt_hint') }}
			</p>

			<div class="rounded-lg border border-border bg-muted/30 p-3 space-y-2.5">
				<p class="text-xs font-semibold text-foreground">
					{{ store.getTranslation('settings.models.editor.variables_panel_title') }}
				</p>
				<p class="text-xs text-muted-foreground">
					{{ store.getTranslation('settings.models.editor.variables_panel_desc') }}
				</p>
				<div class="flex flex-wrap gap-2">
					<div
						v-for="v in systemPromptVariables"
						:key="v.token"
						class="group flex items-start gap-2 rounded-md border border-border bg-background px-2.5 py-1.5 cursor-pointer hover:border-primary/50 hover:bg-primary/5 transition-colors"
						@click="insertVariable(v.token)"
					>
						<code class="text-xs font-mono text-primary whitespace-nowrap">{{ v.token }}</code>
						<span class="text-xs text-muted-foreground leading-tight">{{ store.getTranslation(v.descKey) }}</span>
					</div>
				</div>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import {useMainStore} from '@/stores';

const props = defineProps<{
	form: Record<string, any>;
}>();

const store = useMainStore();
const systemPromptTextareaRef = ref<HTMLTextAreaElement | null>(null);

const systemPromptVariables = [
	{token: '{{user_name}}', descKey: 'settings.models.editor.var_user_name'},
	{token: '{{user_email}}', descKey: 'settings.models.editor.var_user_email'},
	{token: '{{date}}', descKey: 'settings.models.editor.var_date'},
	{token: '{{time}}', descKey: 'settings.models.editor.var_time'},
	{token: '{{datetime}}', descKey: 'settings.models.editor.var_datetime'},
	{token: '{{model_name}}', descKey: 'settings.models.editor.var_model_name'},
	{token: '{{model_id}}', descKey: 'settings.models.editor.var_model_id'},
];

function insertVariable(token: string) {
	const el = systemPromptTextareaRef.value;
	if (el) {
		const start = el.selectionStart ?? props.form.system_prompt.length;
		const end = el.selectionEnd ?? start;
		props.form.system_prompt = props.form.system_prompt.slice(0, start) + token + props.form.system_prompt.slice(end);
		nextTick(() => {
			el.focus();
			el.setSelectionRange(start + token.length, start + token.length);
		});
	} else {
		props.form.system_prompt = props.form.system_prompt + token;
	}
}
</script>
