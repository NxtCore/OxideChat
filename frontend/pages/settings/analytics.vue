<template>
	<SettingsPage>
		<SettingsAnalyticsDashboard :is-admin="true" :user-id="userId" />
	</SettingsPage>
</template>

<script setup lang="ts">
const route = useRoute();
const userId = computed(() => {
	const value = route.query.user_id;
	return Array.isArray(value) ? value[0] : value;
});
</script>
