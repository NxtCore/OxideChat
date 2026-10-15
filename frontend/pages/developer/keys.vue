<template>
	<div class="w-full space-y-4">
		<div class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
			<div>
				<h2 class="text-lg font-semibold text-foreground">{{ store.getTranslation('developer.keys.title') }}</h2>
				<p class="text-sm text-muted-foreground">{{ store.getTranslation('developer.keys.description') }}</p>
			</div>
			<ShadButton :disabled="!developer.teams.length" @click="createOpen = true">
				<Plus class="mr-2 h-4 w-4" />
				{{ store.getTranslation('developer.keys.create') }}
			</ShadButton>
		</div>

		<div v-if="developer.loading && !developer.loaded" class="space-y-3">
			<ShadSkeleton v-for="n in 3" :key="n" class="h-24 w-full" />
		</div>

		<div v-else-if="developer.error" class="flex flex-col items-center gap-3 rounded-lg border border-border bg-card py-12 text-center">
			<AlertTriangle class="h-8 w-8 text-destructive" />
			<p class="font-medium text-foreground">{{ store.getTranslation('developer.errors.load') }}</p>
			<p class="max-w-md text-sm text-muted-foreground">{{ developer.error }}</p>
			<ShadButton variant="outline" @click="reload">{{ store.getTranslation('common.refresh') }}</ShadButton>
		</div>

		<div v-else-if="!developer.keys.length" class="flex flex-col items-center gap-2 rounded-lg border border-dashed border-border bg-card py-16 text-center">
			<KeyRound class="h-8 w-8 text-muted-foreground opacity-60" />
			<p class="font-medium text-foreground">{{ store.getTranslation('developer.keys.empty_title') }}</p>
			<p class="max-w-sm text-sm text-muted-foreground">
				{{ store.getTranslation(developer.teams.length ? 'developer.keys.empty' : 'developer.create.no_teams') }}
			</p>
			<ShadButton v-if="developer.teams.length" class="mt-2" @click="createOpen = true">{{ store.getTranslation('developer.keys.create') }}</ShadButton>
		</div>

		<ul v-else class="space-y-3">
			<li
				v-for="apiKey in developer.keys"
				:key="apiKey.id"
				class="rounded-lg border border-border bg-card p-4"
				:class="keyStatus(apiKey) === 'revoked' ? 'opacity-70' : ''"
				:data-testid="`key-${apiKey.name}`"
			>
				<div class="flex flex-col gap-3">
					<div class="min-w-0 space-y-1">
						<div class="flex flex-wrap items-center gap-2">
							<span class="truncate font-semibold text-foreground">{{ apiKey.name }}</span>
							<DeveloperKeyStatusBadge :api-key="apiKey" />
						</div>
						<p class="font-mono text-sm text-muted-foreground">{{ maskedKey(apiKey) }}</p>
						<p class="text-xs text-muted-foreground">
							{{ developer.teamById(apiKey.team_id)?.name ?? store.getTranslation('developer.keys.unknown_team') }}
							· {{ apiKey.scopes.join(', ') }}
						</p>
						<p class="text-xs text-muted-foreground">
							{{ store.getTranslation('developer.keys.created', {date: formatDate(apiKey.created_at)}) }}
							· {{ apiKey.last_used_at ? store.getTranslation('developer.keys.last_used', {date: formatDate(apiKey.last_used_at)}) : store.getTranslation('developer.keys.never_used') }}
							<template v-if="apiKey.expires_at"> · {{ store.getTranslation('developer.keys.expires', {date: formatDate(apiKey.expires_at)}) }}</template>
							<template v-if="apiKey.max_concurrent_requests"> · {{ store.getTranslation('developer.keys.concurrency', {count: apiKey.max_concurrent_requests}) }}</template>
						</p>
						<p v-if="keyStatus(apiKey) === 'expired'" class="text-xs text-destructive">{{ store.getTranslation('developer.keys.expired_hint') }}</p>
						<p v-if="keyStatus(apiKey) === 'revoked'" class="text-xs text-destructive">
							{{ store.getTranslation('developer.keys.revoked_hint', {date: formatDate(apiKey.revoked_at ?? '')}) }}
						</p>
					</div>

					<div v-if="!apiKey.revoked_at" class="flex flex-wrap items-center gap-2">
						<div class="flex items-center gap-2 pr-2">
							<ShadSwitch
								:model-value="apiKey.is_enabled"
								:aria-label="store.getTranslation('developer.keys.enabled')"
								@update:model-value="toggleEnabled(apiKey, $event === true)"
							/>
							<span class="text-xs text-muted-foreground">{{ store.getTranslation('developer.keys.enabled') }}</span>
						</div>
						<ShadButton variant="outline" size="sm" @click="renameTarget = apiKey; renameValue = apiKey.name">
							<Pencil class="mr-1.5 h-3.5 w-3.5" />{{ store.getTranslation('developer.keys.rename') }}
						</ShadButton>
						<ShadButton variant="outline" size="sm" @click="limitsTarget = apiKey">
							<Gauge class="mr-1.5 h-3.5 w-3.5" />{{ store.getTranslation('developer.keys.limits') }}
						</ShadButton>
						<ShadButton variant="outline" size="sm" @click="budgetTarget = apiKey">
							<Wallet class="mr-1.5 h-3.5 w-3.5" />{{ store.getTranslation('developer.keys.budget') }}
						</ShadButton>
						<ShadButton variant="outline" size="sm" @click="rotateTarget = apiKey">
							<RefreshCw class="mr-1.5 h-3.5 w-3.5" />{{ store.getTranslation('developer.keys.rotate') }}
						</ShadButton>
						<ShadButton variant="outline" size="sm" class="text-destructive hover:text-destructive" @click="revokeTarget = apiKey">
							<Ban class="mr-1.5 h-3.5 w-3.5" />{{ store.getTranslation('developer.keys.revoke') }}
						</ShadButton>
					</div>
				</div>
			</li>
		</ul>

		<DeveloperCreateKeyDialog v-model:open="createOpen" @created="onCreated" />
		<DeveloperRotateKeyDialog :open="!!rotateTarget" :api-key="rotateTarget" @update:open="rotateTarget = null" @rotated="onRotated" />
		<DeveloperKeyLimitsDialog :open="!!limitsTarget" :api-key="limitsTarget" @update:open="limitsTarget = null" />
		<DeveloperKeyBudgetDialog :open="!!budgetTarget" :api-key="budgetTarget" @update:open="budgetTarget = null" />
		<DeveloperKeyRevealDialog
			v-model:open="revealOpen"
			:token="revealToken"
			:title="revealTitle"
			:previous-expires-at="revealPreviousExpiry"
		/>

		<ShadDialog :open="!!renameTarget" @update:open="renameTarget = null">
			<ShadDialogContent class="sm:max-w-[420px]">
				<ShadDialogHeader>
					<ShadDialogTitle>{{ store.getTranslation('developer.keys.rename') }}</ShadDialogTitle>
				</ShadDialogHeader>
				<div class="space-y-1.5 py-2">
					<ShadLabel for="rename-key">{{ store.getTranslation('developer.create.name') }}</ShadLabel>
					<ShadInput id="rename-key" v-model="renameValue" maxlength="100" @keydown.enter="rename" />
				</div>
				<ShadDialogFooter>
					<ShadButton variant="outline" @click="renameTarget = null">{{ store.getTranslation('common.cancel') }}</ShadButton>
					<ShadButton :disabled="!renameValue.trim()" @click="rename">{{ store.getTranslation('common.save') }}</ShadButton>
				</ShadDialogFooter>
			</ShadDialogContent>
		</ShadDialog>

		<ShadDialog :open="!!revokeTarget" @update:open="revokeTarget = null">
			<ShadDialogContent class="sm:max-w-[440px]">
				<ShadDialogHeader>
					<ShadDialogTitle>{{ store.getTranslation('developer.revoke.title') }}</ShadDialogTitle>
					<ShadDialogDescription>{{ store.getTranslation('developer.revoke.description', {name: revokeTarget?.name ?? ''}) }}</ShadDialogDescription>
				</ShadDialogHeader>
				<ShadDialogFooter>
					<ShadButton variant="outline" @click="revokeTarget = null">{{ store.getTranslation('common.cancel') }}</ShadButton>
					<ShadButton variant="destructive" @click="revoke">{{ store.getTranslation('developer.keys.revoke') }}</ShadButton>
				</ShadDialogFooter>
			</ShadDialogContent>
		</ShadDialog>
	</div>
</template>

<script setup lang="ts">
import {AlertTriangle, Ban, Gauge, KeyRound, Pencil, Plus, RefreshCw, Wallet} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {useDeveloperStore} from '@/stores/developerStore';
import type {DeveloperKey, DeveloperKeyRotation, DeveloperKeyWithToken} from '~/types/developer';

const store = useMainStore();
const developer = useDeveloperStore();

const createOpen = ref(false);
const renameTarget = ref<DeveloperKey | null>(null);
const renameValue = ref('');
const rotateTarget = ref<DeveloperKey | null>(null);
const limitsTarget = ref<DeveloperKey | null>(null);
const budgetTarget = ref<DeveloperKey | null>(null);
const revokeTarget = ref<DeveloperKey | null>(null);
const revealOpen = ref(false);
const revealToken = ref('');
const revealTitle = ref('developer.reveal.title_created');
const revealPreviousExpiry = ref<string | null>(null);

function formatDate(value: string) {
	return value ? new Date(value).toLocaleString(store.base?.language ?? 'en') : '';
}

async function reload() {
	await developer.load().catch(() => {});
}

function showToken(token: string, title: string, previousExpiry: string | null) {
	revealToken.value = token;
	revealTitle.value = title;
	revealPreviousExpiry.value = previousExpiry;
	revealOpen.value = true;
}

function onCreated(key: DeveloperKeyWithToken) {
	showToken(key.token, 'developer.reveal.title_created', null);
}

function onRotated(rotation: DeveloperKeyRotation) {
	showToken(rotation.key.token, 'developer.reveal.title_rotated', rotation.previous.expires_at);
}

async function run(action: () => Promise<unknown>, successKey: string | null, errorKey: string) {
	try {
		await action();
		if (successKey) store.toast(store.getTranslation(successKey), {type: 'success'});
		return true;
	} catch (e: any) {
		store.toast(store.getTranslation(errorKey), {type: 'error', description: errorMessage(e, '')});
		return false;
	}
}

function toggleEnabled(apiKey: DeveloperKey, enabled: boolean) {
	return run(() => developer.updateKey(apiKey.id, {is_enabled: enabled}), null, 'developer.keys.update_error');
}

async function rename() {
	const target = renameTarget.value;
	if (!target || !renameValue.value.trim()) return;
	if (await run(() => developer.updateKey(target.id, {name: renameValue.value.trim()}), 'developer.keys.renamed', 'developer.keys.update_error')) {
		renameTarget.value = null;
	}
}

async function revoke() {
	const target = revokeTarget.value;
	if (!target) return;
	if (await run(() => developer.revokeKey(target.id), 'developer.keys.revoked', 'developer.keys.update_error')) {
		revokeTarget.value = null;
	}
}

onMounted(reload);
</script>
