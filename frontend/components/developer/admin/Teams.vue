<template>
	<div class="space-y-4">
		<div v-if="loading" class="space-y-2"><ShadSkeleton v-for="n in 4" :key="n" class="h-14 w-full" /></div>
		<p v-else-if="!teams.length" class="rounded-lg border border-dashed border-border bg-card py-12 text-center text-sm text-muted-foreground">
			{{ store.getTranslation('developer.admin.teams.empty') }}
		</p>
		<div v-else class="overflow-x-auto rounded-lg border border-border bg-card">
			<table class="w-full min-w-[760px] text-sm">
				<thead>
					<tr class="border-b border-border text-left text-xs uppercase tracking-wider text-muted-foreground">
						<th class="px-4 py-3 font-medium">{{ store.getTranslation('developer.admin.keys.team') }}</th>
						<th class="px-4 py-3 font-medium">{{ store.getTranslation('developer.admin.teams.keys') }}</th>
						<th class="px-4 py-3 font-medium">{{ store.getTranslation('developer.admin.teams.limits') }}</th>
						<th class="px-4 py-3 font-medium">{{ store.getTranslation('developer.admin.teams.access') }}</th>
						<th class="px-4 py-3 text-right font-medium">{{ store.getTranslation('developer.admin.actions') }}</th>
					</tr>
				</thead>
				<tbody>
					<tr v-for="team in teams" :key="team.id" class="border-b border-border/60 last:border-b-0">
						<td class="px-4 py-3 text-foreground">{{ team.name }}</td>
						<td class="px-4 py-3 text-muted-foreground">
							{{ store.getTranslation('developer.admin.teams.key_counts', {active: team.active_key_count, total: team.key_count}) }}
						</td>
						<td class="px-4 py-3 text-muted-foreground">
							{{ store.getTranslation('developer.admin.teams.limit_summary', {rules: team.rules.length, cap: team.max_concurrent_requests ?? '–'}) }}
						</td>
						<td class="px-4 py-3">
							<div class="flex items-center gap-2">
								<ShadSwitch :model-value="team.gateway_enabled" :disabled="!canEditAccess" @update:model-value="askAccess(team, $event)" />
								<span class="text-xs" :class="team.gateway_enabled ? 'text-muted-foreground' : 'text-destructive'">
									{{ store.getTranslation(team.gateway_enabled ? 'developer.admin.teams.access_on' : 'developer.admin.teams.access_off') }}
								</span>
							</div>
						</td>
						<td class="px-4 py-3 text-right">
							<ShadButton variant="outline" size="sm" :disabled="!canEditLimits" @click="limitsTarget = team">
								<SlidersHorizontal class="mr-1.5 h-3.5 w-3.5" />{{ store.getTranslation('developer.admin.teams.edit_limits') }}
							</ShadButton>
						</td>
					</tr>
				</tbody>
			</table>
		</div>

		<DeveloperAdminTeamLimitsDialog :open="!!limitsTarget" :team="limitsTarget" @update:open="limitsTarget = null" @saved="replaceTeam" />
		<DeveloperAdminReasonDialog
			:open="!!accessTarget"
			:title="store.getTranslation(accessTarget?.enabled ? 'developer.admin.teams.enable_title' : 'developer.admin.teams.disable_title', {name: accessTarget?.team.name ?? ''})"
			:description="store.getTranslation(accessTarget?.enabled ? 'developer.admin.teams.enable_description' : 'developer.admin.teams.disable_description')"
			:confirm-label="store.getTranslation(accessTarget?.enabled ? 'developer.admin.teams.enable' : 'developer.admin.teams.disable')"
			:destructive="!accessTarget?.enabled"
			:busy="acting"
			@update:open="accessTarget = null"
			@confirm="confirmAccess"
		/>
	</div>
</template>

<script setup lang="ts">
import {ADMIN_DEVELOPER_ACCESS_EDIT, ADMIN_DEVELOPER_LIMITS_EDIT} from '~/types/generated/permissions';
import {SlidersHorizontal} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import {useGatewayAdminStore} from '@/stores/gatewayAdminStore';
import {errorMessage} from '~/utils/format';
import type {AdminTeam} from '~/types/gatewayAdmin';

const store = useMainStore();
const admin = useGatewayAdminStore();

const teams = ref<AdminTeam[]>([]);
const loading = ref(false);
const limitsTarget = ref<AdminTeam | null>(null);
const accessTarget = ref<{team: AdminTeam; enabled: boolean} | null>(null);
const acting = ref(false);

const canEditLimits = computed(() => store.hasPermission(ADMIN_DEVELOPER_LIMITS_EDIT));
const canEditAccess = computed(() => store.hasPermission(ADMIN_DEVELOPER_ACCESS_EDIT));

function replaceTeam(updated: AdminTeam) {
	teams.value = teams.value.map(team => (team.id === updated.id ? updated : team));
}

function askAccess(team: AdminTeam, enabled: boolean) {
	accessTarget.value = {team, enabled};
}

async function confirmAccess(reason: string) {
	if (!accessTarget.value) return;
	acting.value = true;
	try {
		replaceTeam(await admin.setTeamAccess(accessTarget.value.team.id, accessTarget.value.enabled, reason));
		store.toast(store.getTranslation('developer.admin.teams.access_saved'), {type: 'success'});
		accessTarget.value = null;
	} catch (e: any) {
		store.toast(store.getTranslation('developer.admin.teams.access_error'), {type: 'error', description: errorMessage(e, '')});
	} finally {
		acting.value = false;
	}
}

onMounted(async () => {
	loading.value = true;
	try {
		teams.value = (await admin.listTeams()).items;
	} catch (e: any) {
		store.toast(store.getTranslation('developer.admin.teams.load_error'), {type: 'error', description: errorMessage(e, '')});
	} finally {
		loading.value = false;
	}
});
</script>
