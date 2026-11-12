<template>
	<div class="space-y-8">
		<SettingsUsersSection :title="t('roles_title')" test-id="user-roles-section">
			<SettingsAssignmentPicker
				:model-value="targets"
				:options="options"
				:kinds="['role']"
				:disabled="!editable"
				:description="t('roles_description')"
				:describe="describe"
				:empty-title="t('roles_empty')"
				:empty-hint="t('roles_empty_hint')"
				:note="t(isSelf ? 'roles_self_note' : editable ? 'roles_note' : 'readonly_note')"
				test-id="user-roles"
				@update:model-value="value => emit('replace', value)"
			/>
		</SettingsUsersSection>

		<SettingsUsersSection
			:title="t('permissions_title')"
			:description="t('permissions_description')"
			:to="store.hasPermission(ADMIN_ROLES_VIEW) ? '/settings/roles' : undefined"
			:link-label="t('manage_roles')"
			:empty-title="overview.permissions.length ? undefined : t('permissions_empty')"
			:empty-hint="t('permissions_empty_hint')"
			test-id="user-permissions"
		>
			<div class="space-y-4">
				<div v-for="group in groups" :key="group.id" class="space-y-1.5" :data-testid="`user-permission-group-${group.id}`">
					<p class="text-xs font-medium tracking-wide text-muted-foreground uppercase">{{ store.getTranslation(`roles.group.${group.id}`) }}</p>
					<ul class="divide-y divide-border rounded-lg border border-border bg-card">
						<li
							v-for="permission in group.permissions"
							:key="permission.name"
							class="flex flex-wrap items-center gap-x-4 gap-y-1.5 px-3 py-2.5"
							data-testid="user-permission"
							:data-permission="permission.name"
						>
							<span class="min-w-0 flex-1 basis-56">
								<span class="block text-sm text-foreground">{{ store.getTranslation(`roles.permission.${permission.name}`) }}</span>
								<span class="block font-mono text-xs text-muted-foreground" data-allow-key>{{ permission.name }}</span>
							</span>
							<span class="flex flex-wrap gap-1.5">
								<span
									v-for="source in permission.sources"
									:key="source.role"
									class="inline-flex items-center gap-1 rounded-full border border-border px-2 py-0.5 text-xs text-muted-foreground"
									data-testid="user-permission-source"
								>
									<Shield class="h-3 w-3" />
									<span class="text-foreground">{{ source.role }}</span>
									<span v-if="source.through !== permission.name" data-allow-key>{{ t('via', {permission: source.through}) }}</span>
								</span>
							</span>
						</li>
					</ul>
				</div>
			</div>
		</SettingsUsersSection>
	</div>
</template>

<script setup lang="ts">
import {Shield} from 'lucide-vue-next';
import {useMainStore} from '~/stores';
import type {AssignmentOption, AssignmentTarget} from '~/types/assignments';
import {ADMIN_ROLES_VIEW} from '~/types/generated/permissions';
import type {UserOverviewResponse} from '~/types/users';

const props = defineProps<{overview: UserOverviewResponse; targets: AssignmentTarget[]; editable: boolean}>();
const emit = defineEmits<{replace: [targets: AssignmentTarget[]]}>();

const store = useMainStore();
const t = (key: string, args?: Record<string, string>) => store.getTranslation(`settings.user_detail.${key}`, args);

const isSelf = computed(() => props.overview.profile.id === store.auth.user?.id);
const options = computed<AssignmentOption[]>(() => store.roles.map(role => ({kind: 'role', id: role.id, label: role.name})));
const groups = computed(() => groupPermissions(props.overview.permissions));

function describe(target: AssignmentOption): string {
	const role = props.overview.roles.find(held => held.id === target.id);
	return role ? store.getTranslation('roles.permission_count', {count: role.permissions.length}) : '';
}
</script>
