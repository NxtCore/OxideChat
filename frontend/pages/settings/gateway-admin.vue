<template>
	<SettingsPage :title="store.getTranslation('developer.admin.title')" :description="store.getTranslation('developer.admin.description')">
		<ShadTabs v-model="active" class="mb-6">
			<ShadTabsList>
				<ShadTabsTrigger v-for="section in visibleSections" :key="section.id" :value="section.id">{{ store.getTranslation(section.label) }}</ShadTabsTrigger>
			</ShadTabsList>
		</ShadTabs>

		<DeveloperAdminKeys v-if="active === 'keys'" />
		<DeveloperAdminTeams v-else-if="active === 'teams'" />
		<DeveloperAdminAudit v-else-if="active === 'audit'" />
		<DeveloperAdminEvents v-else-if="active === 'events'" />
	</SettingsPage>
</template>

<script setup lang="ts">
import {ADMIN_DEVELOPER_AUDIT_VIEW, ADMIN_DEVELOPER_VIEW} from '~/types/generated/permissions';
import {useMainStore} from '@/stores';

const store = useMainStore();

const sections = [
	{id: 'keys', label: 'developer.admin.tabs.keys', permission: ADMIN_DEVELOPER_VIEW},
	{id: 'teams', label: 'developer.admin.tabs.teams', permission: ADMIN_DEVELOPER_VIEW},
	{id: 'audit', label: 'developer.admin.tabs.audit', permission: ADMIN_DEVELOPER_AUDIT_VIEW},
	{id: 'events', label: 'developer.admin.tabs.events', permission: ADMIN_DEVELOPER_VIEW},
];

const visibleSections = computed(() => sections.filter(section => store.hasPermission(section.permission)));
const active = ref(visibleSections.value[0]?.id ?? 'keys');
</script>
