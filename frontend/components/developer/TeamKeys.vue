<template>
	<div class="w-full space-y-4 pt-4" data-testid="team-keys">
		<div class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
			<div>
				<h2 class="text-lg font-semibold text-foreground">{{ store.getTranslation('developer.service.title') }}</h2>
				<p class="text-sm text-muted-foreground">{{ store.getTranslation('developer.service.description') }}</p>
			</div>
			<ShadButton @click="createOpen = true">
				<Plus class="mr-2 h-4 w-4" />
				{{ store.getTranslation('developer.service.create') }}
			</ShadButton>
		</div>

		<div v-if="!keys.length" class="flex flex-col items-center gap-2 rounded-lg border border-dashed border-border bg-card py-12 text-center">
			<KeyRound class="h-8 w-8 text-muted-foreground opacity-60" />
			<p class="max-w-sm text-sm text-muted-foreground">{{ store.getTranslation('developer.service.empty') }}</p>
		</div>

		<ul v-else class="space-y-3">
			<li
				v-for="apiKey in keys"
				:key="apiKey.id"
				class="rounded-lg border border-border bg-card p-4"
				:class="keyStatus(apiKey) === 'revoked' ? 'opacity-70' : ''"
				:data-testid="`team-key-${apiKey.name}`"
			>
				<div class="flex flex-col gap-3">
					<div class="min-w-0 space-y-1">
						<div class="flex flex-wrap items-center gap-2">
							<span class="truncate font-semibold text-foreground">{{ apiKey.name }}</span>
							<DeveloperKeyStatusBadge :api-key="apiKey" />
							<ShadBadge v-if="apiKey.service_account" variant="outline">{{ store.getTranslation('developer.service.badge') }}</ShadBadge>
						</div>
						<p class="font-mono text-sm text-muted-foreground">{{ maskedKey(apiKey) }}</p>
						<p class="text-xs text-muted-foreground">
							{{ apiKey.team_name }}
							·
							{{ apiKey.service_account ? store.getTranslation('developer.service.owner_team') : store.getTranslation('developer.service.owner_user', {user: apiKey.owner_username ?? ''}) }}
							· {{ apiKey.scopes.join(', ') }}
						</p>
						<p class="text-xs text-muted-foreground">
							{{ store.getTranslation('developer.keys.created', {date: formatDate(apiKey.created_at)}) }}
							· {{ apiKey.last_used_at ? store.getTranslation('developer.keys.last_used', {date: formatDate(apiKey.last_used_at)}) : store.getTranslation('developer.keys.never_used') }}
							<template v-if="apiKey.expires_at"> · {{ store.getTranslation('developer.keys.expires', {date: formatDate(apiKey.expires_at)}) }}</template>
						</p>
						<p v-if="apiKey.admin_disabled" class="text-xs text-destructive">{{ store.getTranslation('developer.keys.admin_disabled_hint') }}</p>
					</div>

					<div v-if="!apiKey.revoked_at" class="flex flex-wrap items-center gap-2">
						<template v-if="apiKey.service_account">
							<ShadButton variant="outline" size="sm" @click="limitsTarget = apiKey">
								<Gauge class="mr-1.5 h-3.5 w-3.5" />{{ store.getTranslation('developer.keys.limits') }}
							</ShadButton>
							<ShadButton variant="outline" size="sm" @click="budgetTarget = apiKey">
								<Wallet class="mr-1.5 h-3.5 w-3.5" />{{ store.getTranslation('developer.keys.budget') }}
							</ShadButton>
							<ShadButton v-if="!apiKey.admin_disabled" variant="outline" size="sm" @click="rotateTarget = apiKey">
								<RefreshCw class="mr-1.5 h-3.5 w-3.5" />{{ store.getTranslation('developer.keys.rotate') }}
							</ShadButton>
						</template>
						<ShadButton variant="outline" size="sm" class="text-destructive hover:text-destructive" @click="revokeTarget = apiKey">
							<Ban class="mr-1.5 h-3.5 w-3.5" />{{ store.getTranslation('developer.keys.revoke') }}
						</ShadButton>
					</div>
				</div>
			</li>
		</ul>

		<DeveloperCreateKeyDialog v-model:open="createOpen" service-account @created="onCreated" />
		<DeveloperRotateKeyDialog :open="!!rotateTarget" :api-key="rotateTarget" @update:open="rotateTarget = null" @rotated="onRotated" />
		<DeveloperKeyLimitsDialog :open="!!limitsTarget" :api-key="limitsTarget" @update:open="limitsTarget = null" />
		<DeveloperKeyBudgetDialog :open="!!budgetTarget" :api-key="budgetTarget" @update:open="budgetTarget = null" />
		<DeveloperKeyRevealDialog v-model:open="revealOpen" :token="revealToken" :title="revealTitle" :previous-expires-at="revealPreviousExpiry" />

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
import {Ban, Gauge, KeyRound, Plus, RefreshCw, Wallet} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {useDeveloperStore} from '@/stores/developerStore';
import type {DeveloperKey, DeveloperKeyRotation, DeveloperKeyWithToken, DeveloperTeamKey} from '~/types/developer';

const store = useMainStore();
const developer = useDeveloperStore();

const createOpen = ref(false);
const rotateTarget = ref<DeveloperKey | null>(null);
const limitsTarget = ref<DeveloperKey | null>(null);
const budgetTarget = ref<DeveloperKey | null>(null);
const revokeTarget = ref<DeveloperKey | null>(null);
const revealOpen = ref(false);
const revealToken = ref('');
const revealTitle = ref('developer.reveal.title_created');
const revealPreviousExpiry = ref<string | null>(null);

const keys = computed<DeveloperTeamKey[]>(() => developer.teamKeys.filter(key => key.service_account || key.user_id !== store.auth.user?.id));

function formatDate(value: string) {
	return value ? new Date(value).toLocaleString(store.base?.language ?? 'en') : '';
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

async function revoke() {
	const target = revokeTarget.value;
	if (!target) return;
	try {
		await developer.revokeKey(target.id);
		store.toast(store.getTranslation('developer.keys.revoked'), {type: 'success'});
		revokeTarget.value = null;
	} catch (e: any) {
		store.toast(store.getTranslation('developer.keys.update_error'), {type: 'error', description: errorMessage(e, '')});
	}
}
</script>
