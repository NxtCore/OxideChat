<template>
	<div class="flex min-h-screen items-center justify-center bg-background p-4">
		<div class="flex w-full max-w-md flex-col gap-6">
			<ShadCard>
				<ShadCardHeader class="text-center">
					<ShadCardTitle class="text-xl">{{ store.getTranslation('auth.setup.title') }}</ShadCardTitle>
					<ShadCardDescription>{{ store.getTranslation('auth.setup.description') }}</ShadCardDescription>
				</ShadCardHeader>
				<ShadCardContent>
					<form @submit.prevent="handleSetup" class="flex flex-col gap-6">
						<!-- Form Fields -->
						<div class="flex flex-col gap-4">
							<div class="flex flex-col gap-2">
								<ShadLabel for="email">{{ store.getTranslation('auth.login.email') }}</ShadLabel>
								<ShadInput id="email" v-model="form.email" type="email" placeholder="admin@example.com" required />
							</div>
							<div class="flex flex-col gap-2">
								<ShadLabel for="username">{{ store.getTranslation('auth.register.username') }}</ShadLabel>
								<ShadInput id="username" v-model="form.username" type="text" placeholder="admin" required />
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

						<!-- Error Message -->
						<div v-if="error" class="rounded-md bg-destructive/10 p-3 text-sm text-destructive">
							{{ error }}
						</div>

						<!-- Submit -->
						<ShadButton type="submit" class="w-full" :disabled="loading">
							<Loader2 v-if="loading" class="mr-2 h-4 w-4 animate-spin" />
							{{ loading ? store.getTranslation('auth.register.submitting') : store.getTranslation('auth.setup.submit') }}
						</ShadButton>
					</form>
				</ShadCardContent>
			</ShadCard>

			<p class="px-6 text-center text-xs text-muted-foreground">{{ store.getTranslation('auth.setup.one_time_tip') }}</p>
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
const error = ref('');
const passwordStrength = ref<{isValid: boolean} | null>(null);

async function handleSetup() {
	error.value = '';

	if (form.value.password !== form.value.confirmPassword) {
		error.value = store.getTranslation('auth.register.passwords_mismatch');
		return;
	}

	// Use password strength component validation
	if (!passwordStrength.value?.isValid) {
		error.value = store.getTranslation('auth.errors.password_requirements_not_met');
		return;
	}

	loading.value = true;

	try {
		await store.setup(form.value.email, form.value.username, form.value.password);
		router.push('/');
	} catch (e: any) {
		error.value = e.message || store.getTranslation('auth.errors.internal_error');
	} finally {
		loading.value = false;
	}
}
</script>
