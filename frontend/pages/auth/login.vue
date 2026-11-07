<template>
	<div class="bg-muted flex min-h-svh flex-col items-center justify-center gap-6 p-6 md:p-10">
		<div class="flex w-full max-w-md flex-col gap-6">
			<ShadCard>
				<ShadCardHeader class="text-center">
					<ShadCardTitle class="text-xl">{{ store.getTranslation('auth.login.title') }}</ShadCardTitle>
					<ShadCardDescription>{{ store.getTranslation('auth.login.description') }}</ShadCardDescription>
				</ShadCardHeader>
				<ShadCardContent>
					<form @submit.prevent="handleLogin" class="flex flex-col gap-6">
						<AuthOAuthButtons />

						<div v-if="store.base?.oauth_providers.length > 0" class="relative flex items-center">
							<div class="flex-1 border-t border-border"></div>
							<span class="bg-card px-4 text-xs text-muted-foreground">{{ store.getTranslation('auth.login.or_continue') }}</span>
							<div class="flex-1 border-t border-border"></div>
						</div>

						<div class="flex flex-col gap-4">
							<div class="flex flex-col gap-2">
								<ShadLabel for="email">{{ store.getTranslation('auth.login.email') }}</ShadLabel>
								<ShadInput id="email" v-model="form.email" type="email" placeholder="m@example.com" required />
							</div>
							<div class="flex flex-col gap-2">
								<div class="flex items-center">
									<ShadLabel for="password">{{ store.getTranslation('auth.login.password') }}</ShadLabel>
								</div>
								<ShadInput id="password" v-model="form.password" type="password" required />
							</div>
						</div>

						<div class="flex flex-col gap-4">
							<ShadButton type="submit" class="w-full" :disabled="loading">
								<Loader2 v-if="loading" class="h-4 w-4 animate-spin" />
								{{ loading ? store.getTranslation('auth.login.submitting') : store.getTranslation('auth.login.submit') }}
							</ShadButton>
							<p class="text-center text-sm text-muted-foreground">
								{{ store.getTranslation('auth.login.no_account') }}
								<NuxtLink to="/auth/register" class="underline underline-offset-4 hover:text-primary">{{
									store.getTranslation('auth.login.sign_up')
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
	password: '',
});

const loading = ref(false);

async function handleLogin() {
	loading.value = true;

	try {
		await store.login(form.value.email, form.value.password);
		router.push('/');
	} catch (e: any) {
		if (isServerUnreachable(e)) {
			store.toast('Server is starting up, please wait…', {type: 'warning'});
		} else {
			store.toast(e.message || store.getTranslation('auth.errors.invalid_email_or_password'), {type: 'error'});
		}
	} finally {
		loading.value = false;
	}
}
</script>
