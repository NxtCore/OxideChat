<template>
	<div class="flex min-h-screen items-center justify-center bg-background p-4">
		<div class="flex w-full max-w-md flex-col gap-6">
			<ShadCard>
				<ShadCardHeader class="text-center">
					<ShadCardTitle class="text-xl">{{ store.getTranslation('auth.register.title') }}</ShadCardTitle>
					<ShadCardDescription>{{ store.getTranslation('auth.register.description') }}</ShadCardDescription>
				</ShadCardHeader>
				<ShadCardContent>
					<form @submit.prevent="handleRegister" class="flex flex-col gap-6">
						<!-- OAuth Buttons -->
						<div v-if="store.base?.oauth_providers.length > 0" class="flex flex-col gap-3">
							<ShadButton v-if="store.isOAuthEnabled('google')" variant="outline" type="button" class="w-full" @click="handleOauthRegister('google')">
								<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" class="size-5">
									<path
										d="M12.48 10.92v3.28h7.84c-.24 1.84-.853 3.187-1.787 4.133-1.147 1.147-2.933 2.4-6.053 2.4-4.827 0-8.6-3.893-8.6-8.72s3.773-8.72 8.6-8.72c2.6 0 4.507 1.027 5.907 2.347l2.307-2.307C18.747 1.44 16.133 0 12.48 0 5.867 0 .307 5.387.307 12s5.56 12 12.173 12c3.573 0 6.267-1.173 8.373-3.36 2.16-2.16 2.84-5.213 2.84-7.667 0-.76-.053-1.467-.173-2.053H12.48z"
										fill="currentColor"
									/>
								</svg>
								{{ store.getTranslation('auth.register.google') }}
							</ShadButton>
							<ShadButton v-if="store.isOAuthEnabled('discord')" variant="outline" type="button" class="w-full" @click="handleOauthRegister('discord')">
								<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" class="size-5">
									<path
										d="M20.317 4.37a19.791 19.791 0 0 0-4.885-1.515.074.074 0 0 0-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 0 0-5.487 0 12.64 12.64 0 0 0-.617-1.25.077.077 0 0 0-.079-.037A19.736 19.736 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 0 0 .031.057 19.9 19.9 0 0 0 5.993 3.03.078.078 0 0 0 .084-.028 14.09 14.09 0 0 0 1.226-1.994.076.076 0 0 0-.041-.106 13.107 13.107 0 0 1-1.872-.892.077.077 0 0 1-.008-.128 10.2 10.2 0 0 0 .372-.292.074.074 0 0 1 .077-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 0 1 .078.01c.12.098.246.198.373.292a.077.077 0 0 1-.006.127 12.299 12.299 0 0 1-1.873.892.077.077 0 0 0-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.028 19.839 19.839 0 0 0 6.002-3.03.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.06.06 0 0 0-.031-.03zM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.955-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.956 2.418-2.157 2.418zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.955-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.946 2.418-2.157 2.418z"
										fill="currentColor"
									/>
								</svg>
								{{ store.getTranslation('auth.register.discord') }}
							</ShadButton>
						</div>

						<!-- Separator -->
						<div v-if="store.base?.oauth_providers.length > 0" class="relative flex items-center">
							<div class="flex-1 border-t border-border"></div>
							<span class="bg-card px-4 text-xs text-muted-foreground">{{ store.getTranslation('auth.login.or_continue') }}</span>
							<div class="flex-1 border-t border-border"></div>
						</div>

						<!-- Form Fields -->
						<div class="flex flex-col gap-4">
							<div class="flex flex-col gap-2">
								<ShadLabel for="email">{{ store.getTranslation('auth.login.email') }}</ShadLabel>
								<ShadInput id="email" v-model="form.email" type="email" placeholder="m@example.com" required />
							</div>
							<div class="flex flex-col gap-2">
								<ShadLabel for="username">{{ store.getTranslation('auth.register.username') }}</ShadLabel>
								<ShadInput id="username" v-model="form.username" type="text" placeholder="yourname" required />
							</div>
							<div class="flex flex-col gap-2">
								<ShadLabel for="password">{{ store.getTranslation('auth.login.password') }}</ShadLabel>
								<ShadInput
									id="password"
									v-model="form.password"
									type="password"
									:placeholder="store.getTranslation('auth.register.password_requirements')"
									required
								/>
								<PasswordStrengthIndicator ref="passwordStrength" :password="form.password" />
							</div>
							<div class="flex flex-col gap-2">
								<ShadLabel for="confirmPassword">{{ store.getTranslation('auth.register.confirm_password') }}</ShadLabel>
								<ShadInput id="confirmPassword" v-model="form.confirmPassword" type="password" required />
							</div>
						</div>

						<!-- Submit -->
						<div class="flex flex-col gap-4">
							<ShadButton type="submit" class="w-full" :disabled="loading">
								<Loader2 v-if="loading" class="mr-2 h-4 w-4 animate-spin" />
								{{ loading ? store.getTranslation('auth.register.submitting') : store.getTranslation('auth.register.submit') }}
							</ShadButton>
							<p class="text-center text-sm text-muted-foreground">
								{{ store.getTranslation('auth.register.have_account') }}
								<NuxtLink to="/auth/login" class="underline underline-offset-4 hover:text-primary">{{
									store.getTranslation('auth.register.sign_in')
								}}</NuxtLink>
							</p>
						</div>
					</form>
				</ShadCardContent>
			</ShadCard>
		</div>
	</div>
</template>

<script setup lang="ts">
import {Loader2} from 'lucide-vue-next';
import {useMainStore} from '@/stores';

const store = useMainStore();
const router = useRouter();

const form = ref({
	email: '',
	username: '',
	password: '',
	confirmPassword: '',
});

const loading = ref(false);
const passwordStrength = ref<{isValid: boolean} | null>(null);

async function handleRegister() {
	if (form.value.password !== form.value.confirmPassword) {
		store.toast(store.getTranslation('auth.register.passwords_mismatch'), {type: 'error'});
		return;
	}

	// Use password strength component validation
	if (!passwordStrength.value?.isValid) {
		store.toast(store.getTranslation('auth.errors.password_requirements_not_met'), {type: 'error'});
		return;
	}

	loading.value = true;

	try {
		await store.register(form.value.email, form.value.username, form.value.password);
		router.push('/');
	} catch (e: any) {
		store.toast(e.message || store.getTranslation('auth.errors.internal_error'), {type: 'error'});
	} finally {
		loading.value = false;
	}
}

function handleOauthRegister(provider: string) {
	// Redirect to backend OAuth init endpoint - OAuth handles both login and register
	window.location.href = `/api/v1/auth/oauth/${provider}`;
}
</script>
