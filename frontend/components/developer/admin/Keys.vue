<template>
	<div class="space-y-4">
		<div class="grid grid-cols-1 gap-3 rounded-lg border border-border bg-card p-4 sm:grid-cols-2 lg:grid-cols-5">
			<div class="space-y-1.5">
				<ShadLabel>{{ store.getTranslation('developer.admin.keys.search') }}</ShadLabel>
				<ShadInput v-model="search" :placeholder="store.getTranslation('developer.admin.keys.search_placeholder')" @keyup.enter="reload" />
			</div>
			<div class="space-y-1.5">
				<ShadLabel>{{ store.getTranslation('developer.admin.keys.team') }}</ShadLabel>
				<ShadSelect v-model="teamFilter" @update:model-value="reload">
					<ShadSelectTrigger class="w-full"><ShadSelectValue /></ShadSelectTrigger>
					<ShadSelectContent>
						<ShadSelectItem value="all">{{ store.getTranslation('developer.logs.all_teams') }}</ShadSelectItem>
						<ShadSelectItem v-for="team in teams" :key="team.id" :value="team.id">{{ team.name }}</ShadSelectItem>
					</ShadSelectContent>
				</ShadSelect>
			</div>
			<div class="space-y-1.5">
				<ShadLabel>{{ store.getTranslation('developer.admin.keys.status') }}</ShadLabel>
				<ShadSelect v-model="statusFilter" @update:model-value="reload">
					<ShadSelectTrigger class="w-full"><ShadSelectValue /></ShadSelectTrigger>
					<ShadSelectContent>
						<ShadSelectItem value="all">{{ store.getTranslation('developer.logs.all_statuses') }}</ShadSelectItem>
						<ShadSelectItem v-for="status in ADMIN_KEY_STATUSES" :key="status" :value="status">{{ store.getTranslation(`developer.status.${status}`) }}</ShadSelectItem>
					</ShadSelectContent>
				</ShadSelect>
			</div>
			<div class="space-y-1.5">
				<ShadLabel>{{ store.getTranslation('developer.admin.keys.expires_before') }}</ShadLabel>
				<ShadInput v-model="expiresTo" type="date" @change="reload" />
			</div>
			<div class="space-y-1.5">
				<ShadLabel>{{ store.getTranslation('developer.admin.keys.last_used_before') }}</ShadLabel>
				<ShadInput v-model="lastUsedBefore" type="date" @change="reload" />
			</div>
			<div class="flex items-center gap-2 sm:col-span-2 lg:col-span-5">
				<ShadSwitch :model-value="neverUsed" @update:model-value="setNeverUsed" />
				<ShadLabel>{{ store.getTranslation('developer.admin.keys.never_used') }}</ShadLabel>
				<ShadButton variant="outline" size="sm" class="ml-auto" @click="reload">
					<RefreshCw class="mr-1.5 h-3.5 w-3.5" />{{ store.getTranslation('common.refresh') }}
				</ShadButton>
			</div>
		</div>

		<div v-if="loading" class="space-y-2"><ShadSkeleton v-for="n in 5" :key="n" class="h-12 w-full" /></div>
		<p v-else-if="!keys.length" class="rounded-lg border border-dashed border-border bg-card py-12 text-center text-sm text-muted-foreground">
			{{ store.getTranslation('developer.admin.keys.empty') }}
		</p>
		<div v-else class="overflow-x-auto rounded-lg border border-border bg-card">
			<table class="w-full min-w-[900px] text-sm">
				<thead>
					<tr class="border-b border-border text-left text-xs uppercase tracking-wider text-muted-foreground">
						<th class="px-4 py-3 font-medium">{{ store.getTranslation('developer.admin.keys.key') }}</th>
						<th class="px-4 py-3 font-medium">{{ store.getTranslation('developer.admin.keys.owner') }}</th>
						<th class="px-4 py-3 font-medium">{{ store.getTranslation('developer.admin.keys.team') }}</th>
						<th class="px-4 py-3 font-medium">{{ store.getTranslation('developer.admin.keys.status') }}</th>
						<th class="px-4 py-3 font-medium">{{ store.getTranslation('developer.admin.keys.expires') }}</th>
						<th class="px-4 py-3 font-medium">{{ store.getTranslation('developer.admin.keys.last_used') }}</th>
						<th class="px-4 py-3 text-right font-medium">{{ store.getTranslation('developer.admin.actions') }}</th>
					</tr>
				</thead>
				<tbody>
					<tr v-for="apiKey in keys" :key="apiKey.id" class="border-b border-border/60 last:border-b-0">
						<td class="px-4 py-3">
							<p class="text-foreground">{{ apiKey.name }}</p>
							<p class="font-mono text-xs text-muted-foreground">{{ apiKey.key_prefix }}…{{ apiKey.last_four }}</p>
						</td>
						<td class="px-4 py-3 text-muted-foreground">{{ apiKey.owner_username }}</td>
						<td class="px-4 py-3 text-muted-foreground">{{ apiKey.team_name }}</td>
						<td class="px-4 py-3">
							<ShadBadge :variant="apiKey.status === 'active' ? 'default' : apiKey.status === 'revoked' ? 'destructive' : 'secondary'">
								{{ store.getTranslation(`developer.status.${apiKey.status}`) }}
							</ShadBadge>
						</td>
						<td class="whitespace-nowrap px-4 py-3 text-muted-foreground">{{ formatDate(apiKey.expires_at) }}</td>
						<td class="whitespace-nowrap px-4 py-3 text-muted-foreground">{{ formatDate(apiKey.last_used_at) }}</td>
						<td class="px-4 py-3">
							<div class="flex justify-end gap-1">
								<ShadButton variant="ghost" size="icon" :aria-label="store.getTranslation('developer.admin.keys.inspect')" @click="inspectId = apiKey.id">
									<Eye class="h-4 w-4" />
								</ShadButton>
								<template v-if="canEdit && apiKey.status !== 'revoked'">
									<ShadButton
										variant="ghost"
										size="icon"
										:aria-label="store.getTranslation(apiKey.admin_disabled ? 'developer.admin.keys.enable' : 'developer.admin.keys.disable')"
										@click="ask(apiKey, apiKey.admin_disabled ? 'enable' : 'disable')"
									>
										<component :is="apiKey.admin_disabled ? CheckCircle2 : Ban" class="h-4 w-4" />
									</ShadButton>
									<ShadButton variant="ghost" size="icon" :aria-label="store.getTranslation('developer.admin.keys.revoke')" @click="ask(apiKey, 'revoke')">
										<Trash2 class="h-4 w-4 text-destructive" />
									</ShadButton>
								</template>
							</div>
						</td>
					</tr>
				</tbody>
			</table>
			<div class="flex items-center justify-between border-t border-border px-4 py-2">
				<ShadButton variant="outline" size="sm" :disabled="page <= 1" @click="goto(page - 1)">{{ store.getTranslation('common.previous') }}</ShadButton>
				<span class="text-xs text-muted-foreground">{{ page }}</span>
				<ShadButton variant="outline" size="sm" :disabled="!hasMore" @click="goto(page + 1)">{{ store.getTranslation('common.next') }}</ShadButton>
			</div>
		</div>

		<DeveloperAdminKeyDetailDialog :open="!!inspectId" :key-id="inspectId" @update:open="inspectId = null" />
		<DeveloperAdminReasonDialog
			:open="!!pending"
			:title="store.getTranslation(`developer.admin.keys.${pending?.action ?? 'revoke'}_title`, {name: pending?.key.name ?? ''})"
			:description="store.getTranslation(`developer.admin.keys.${pending?.action ?? 'revoke'}_description`)"
			:confirm-label="store.getTranslation(`developer.admin.keys.${pending?.action ?? 'revoke'}`)"
			:destructive="pending?.action === 'revoke'"
			:busy="acting"
			@update:open="pending = null"
			@confirm="confirmAction"
		/>
	</div>
</template>

<script setup lang="ts">
import {Ban, CheckCircle2, Eye, RefreshCw, Trash2} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {useGatewayAdminStore} from '@/stores/gatewayAdminStore';
import {errorMessage} from '~/utils/developer';
import {ADMIN_KEY_STATUSES} from '~/types/gatewayAdmin';
import type {AdminKey, AdminKeyFilters, AdminKeyStatus, AdminTeam} from '~/types/gatewayAdmin';

const store = useMainStore();
const admin = useGatewayAdminStore();

const keys = ref<AdminKey[]>([]);
const teams = ref<AdminTeam[]>([]);
const hasMore = ref(false);
const page = ref(1);
const loading = ref(false);
const search = ref('');
const teamFilter = ref('all');
const statusFilter = ref('all');
const expiresTo = ref('');
const lastUsedBefore = ref('');
const neverUsed = ref(false);
const inspectId = ref<string | null>(null);
const pending = ref<{key: AdminKey; action: 'revoke' | 'disable' | 'enable'} | null>(null);
const acting = ref(false);

const canEdit = computed(() => store.hasPermission('admin.developer.keys.edit'));

function formatDate(value: string | null): string {
	return value ? new Date(value).toLocaleString(store.base?.language ?? 'en') : '–';
}

function endOfDay(value: string): string | null {
	return value ? new Date(`${value}T23:59:59`).toISOString() : null;
}

function filters(): AdminKeyFilters {
	return {
		team_id: teamFilter.value === 'all' ? null : teamFilter.value,
		status: statusFilter.value === 'all' ? null : (statusFilter.value as AdminKeyStatus),
		expires_to: endOfDay(expiresTo.value),
		last_used_before: endOfDay(lastUsedBefore.value),
		never_used: neverUsed.value || null,
		search: search.value.trim() || null,
	};
}

async function load() {
	loading.value = true;
	try {
		const result = await admin.listKeys(filters(), page.value);
		keys.value = result.items;
		hasMore.value = result.has_more;
	} catch (e: any) {
		store.toast(store.getTranslation('developer.admin.keys.load_error'), {type: 'error', description: errorMessage(e, '')});
	} finally {
		loading.value = false;
	}
}

async function reload() {
	page.value = 1;
	await load();
}

function goto(next: number) {
	page.value = Math.max(1, next);
	load();
}

function setNeverUsed(value: boolean) {
	neverUsed.value = value;
	reload();
}

function ask(key: AdminKey, action: 'revoke' | 'disable' | 'enable') {
	pending.value = {key, action};
}

async function confirmAction(reason: string) {
	if (!pending.value) return;
	acting.value = true;
	try {
		await admin.keyAction(pending.value.key.id, pending.value.action, reason);
		store.toast(store.getTranslation('developer.admin.keys.action_done'), {type: 'success'});
		pending.value = null;
		await load();
	} catch (e: any) {
		store.toast(store.getTranslation('developer.admin.keys.action_error'), {type: 'error', description: errorMessage(e, '')});
	} finally {
		acting.value = false;
	}
}

onMounted(async () => {
	admin
		.listTeams()
		.then(result => {
			teams.value = result.items;
		})
		.catch(() => {});
	await load();
});
</script>
