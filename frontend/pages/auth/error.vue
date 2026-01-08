<template>
	<div class="bg-muted flex min-h-svh flex-col items-center justify-center gap-6 p-6 md:p-10">
		<div class="flex w-full max-w-md flex-col gap-6">
			<ShadCard>
				<ShadCardHeader class="text-center">
					<div class="mx-auto mb-4 flex h-12 w-12 items-center justify-center rounded-full bg-destructive/10">
						<AlertCircle class="h-6 w-6 text-destructive" />
					</div>
					<ShadCardTitle class="text-xl">{{ errorTitle }}</ShadCardTitle>
					<ShadCardDescription>{{ errorDescription }}</ShadCardDescription>
				</ShadCardHeader>
				<ShadCardContent>
					<div class="flex flex-col gap-4">
						<div v-if="errorCode === 'oauth_email_conflict'" class="rounded-lg bg-muted p-4 text-sm">
							<p class="text-muted-foreground">{{ store.getTranslation('auth.errors.oauth_email_conflict_help') }}</p>
						</div>

						<div class="flex flex-col gap-2">
							<ShadButton @click="navigateTo('/auth/login')" class="w-full">
								{{ store.getTranslation('auth.errors.back_to_login') }}
							</ShadButton>
							<ShadButton variant="outline" @click="navigateTo('/')" class="w-full">
								{{ store.getTranslation('auth.errors.go_home') }}
							</ShadButton>
						</div>
					</div>
				</ShadCardContent>
			</ShadCard>
		</div>
	</div>
</template>

<script setup lang="ts">
import {AlertCircle} from 'lucide-vue-next';
import {useMainStore} from '@/stores';

const store = useMainStore();
const route = useRoute();

// Get error code from query params
const errorCode = computed(() => {
	return (route.query.code as string) || 'unknown';
});

// Map error codes to i18n keys for titles
const errorTitle = computed(() => {
	const titleKeys: Record<string, string> = {
		oauth_email_conflict: 'auth.errors.oauth_email_conflict_title',
		oauth_state_mismatch: 'auth.errors.oauth_state_mismatch_title',
		oauth_token_error: 'auth.errors.oauth_token_error_title',
		oauth_user_info_error: 'auth.errors.oauth_user_info_error_title',
		unknown: 'auth.errors.unknown_error_title',
	};
	return store.getTranslation(titleKeys[errorCode.value] || titleKeys.unknown);
});

// Map error codes to i18n keys for descriptions
const errorDescription = computed(() => {
	const descKeys: Record<string, string> = {
		oauth_email_conflict: 'auth.errors.oauth_email_conflict',
		oauth_state_mismatch: 'auth.errors.oauth_state_mismatch',
		oauth_token_error: 'auth.errors.oauth_token_error',
		oauth_user_info_error: 'auth.errors.oauth_user_info_error',
		unknown: 'auth.errors.unknown_error',
	};
	return store.getTranslation(descKeys[errorCode.value] || descKeys.unknown);
});
</script>
