<template>
	<ShadDialog :open="open" @update:open="emit('update:open', $event)">
		<ShadDialogContent class="max-h-[90dvh] overflow-y-auto sm:max-w-[520px]">
			<ShadDialogHeader>
				<ShadDialogTitle>{{ store.getTranslation('developer.create.title') }}</ShadDialogTitle>
				<ShadDialogDescription>{{ store.getTranslation('developer.create.description') }}</ShadDialogDescription>
			</ShadDialogHeader>
			<div class="space-y-4 py-2">
				<div class="space-y-1.5">
					<ShadLabel for="key-name">{{ store.getTranslation('developer.create.name') }}</ShadLabel>
					<ShadInput id="key-name" v-model="form.name" maxlength="100" :placeholder="store.getTranslation('developer.create.name_placeholder')" />
				</div>

				<div class="space-y-1.5">
					<ShadLabel>{{ store.getTranslation('developer.create.team') }}</ShadLabel>
					<p v-if="!developer.teams.length" class="rounded-md border border-dashed border-border p-3 text-sm text-muted-foreground">
						{{ store.getTranslation('developer.create.no_teams') }}
					</p>
					<div v-else class="space-y-2">
						<button
							v-for="team in developer.teams"
							:key="team.id"
							type="button"
							class="w-full rounded-md border p-3 text-left transition-colors"
							:class="form.team_id === team.id ? 'border-primary bg-accent' : 'border-border hover:bg-accent/50'"
							@click="form.team_id = team.id"
						>
							<div class="flex items-center justify-between gap-2">
								<span class="font-medium text-foreground">{{ team.name }}</span>
								<span class="text-xs text-muted-foreground">{{ store.getTranslation('developer.models_count', {count: team.models.length}) }}</span>
							</div>
							<p class="mt-1 line-clamp-2 break-words text-xs text-muted-foreground">
								{{ team.models.length ? team.models.join(', ') : store.getTranslation('developer.no_models') }}
							</p>
						</button>
					</div>
				</div>

				<div class="space-y-2">
					<ShadLabel>{{ store.getTranslation('developer.create.scopes') }}</ShadLabel>
					<div v-for="scope in KEY_SCOPES" :key="scope" class="flex items-start gap-3">
						<ShadCheckbox :id="`scope-${scope}`" :model-value="form.scopes.includes(scope)" class="mt-0.5" @update:model-value="toggleScope(scope, $event === true)" />
						<ShadLabel :for="`scope-${scope}`" class="cursor-pointer flex-col items-start gap-0.5 leading-snug">
							<span class="font-mono text-sm">{{ scope }}</span>
							<span class="text-xs font-normal text-muted-foreground">{{ store.getTranslation(`developer.scope.${scope}`) }}</span>
						</ShadLabel>
					</div>
				</div>

				<div class="grid gap-4 sm:grid-cols-2">
					<div class="space-y-1.5">
						<ShadLabel for="key-expiry">{{ store.getTranslation('developer.create.expiry') }}</ShadLabel>
						<ShadInput id="key-expiry" v-model="form.expires_at" type="datetime-local" />
						<p class="text-xs text-muted-foreground">{{ store.getTranslation('developer.create.expiry_hint') }}</p>
					</div>
					<div class="space-y-1.5">
						<ShadLabel for="key-concurrency">{{ store.getTranslation('developer.create.concurrency') }}</ShadLabel>
						<ShadInput id="key-concurrency" v-model="form.max_concurrent_requests" type="number" min="1" />
						<p class="text-xs text-muted-foreground">{{ store.getTranslation('developer.create.concurrency_hint') }}</p>
					</div>
				</div>
			</div>
			<ShadDialogFooter>
				<ShadButton variant="outline" @click="emit('update:open', false)">{{ store.getTranslation('common.cancel') }}</ShadButton>
				<ShadButton :disabled="!canSubmit" @click="submit">
					<Loader2 v-if="saving" class="mr-2 h-4 w-4 animate-spin" />
					{{ store.getTranslation('developer.create.submit') }}
				</ShadButton>
			</ShadDialogFooter>
		</ShadDialogContent>
	</ShadDialog>
</template>

<script setup lang="ts">
import {Loader2} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {useDeveloperStore} from '@/stores/developerStore';
import {KEY_SCOPES, type DeveloperKeyWithToken} from '~/types/developer';

const props = defineProps<{open: boolean}>();
const emit = defineEmits<{'update:open': [value: boolean]; created: [key: DeveloperKeyWithToken]}>();

const store = useMainStore();
const developer = useDeveloperStore();
const saving = ref(false);

function emptyForm() {
	return {
		name: '',
		team_id: developer.teams.find(team => team.is_default)?.id ?? developer.teams[0]?.id ?? '',
		scopes: [...KEY_SCOPES] as string[],
		expires_at: '',
		max_concurrent_requests: '' as string | number,
	};
}

const form = reactive(emptyForm());

watch(
	() => props.open,
	(value) => {
		if (value) Object.assign(form, emptyForm());
	}
);

const canSubmit = computed(() => !saving.value && form.name.trim().length > 0 && form.team_id !== '' && form.scopes.length > 0);

function toggleScope(scope: string, checked: boolean) {
	form.scopes = checked ? [...form.scopes, scope] : form.scopes.filter(existing => existing !== scope);
}

async function submit() {
	saving.value = true;
	try {
		const concurrency = Number(form.max_concurrent_requests);
		const created = await developer.createKey({
			name: form.name.trim(),
			team_id: form.team_id,
			scopes: form.scopes,
			expires_at: form.expires_at ? new Date(form.expires_at).toISOString() : null,
			max_concurrent_requests: concurrency > 0 ? concurrency : null,
		});
		emit('update:open', false);
		emit('created', created);
	} catch (e: any) {
		store.toast(store.getTranslation('developer.create.error'), {type: 'error', description: errorMessage(e, '')});
	} finally {
		saving.value = false;
	}
}
</script>
