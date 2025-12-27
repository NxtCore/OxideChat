<template>
	<div class="flex min-h-screen items-center justify-center bg-gradient-to-br from-zinc-900 via-zinc-800 to-zinc-900">
		<div class="w-full max-w-md space-y-8 px-4">
			<!-- Register Card -->
			<ShadCard class="border-zinc-700 bg-zinc-800/50 backdrop-blur">
				<ShadCardHeader class="text-center">
					<div class="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-full bg-gradient-to-br from-amber-400 to-orange-500">
						<UserPlus class="h-8 w-8 text-white" />
					</div>
					<ShadCardTitle class="text-2xl font-bold text-white">Create Account</ShadCardTitle>
					<ShadCardDescription class="text-zinc-400"> Join OxideChat to start chatting </ShadCardDescription>
				</ShadCardHeader>
				<ShadCardContent>
					<form @submit.prevent="handleRegister" class="space-y-4">
						<div class="space-y-2">
							<ShadLabel for="email" class="text-zinc-300">Email</ShadLabel>
							<ShadInput
								id="email"
								v-model="form.email"
								type="email"
								placeholder="you@example.com"
								required
								class="border-zinc-600 bg-zinc-700/50 text-white placeholder:text-zinc-500"
							/>
						</div>
						<div class="space-y-2">
							<ShadLabel for="username" class="text-zinc-300">Username</ShadLabel>
							<ShadInput
								id="username"
								v-model="form.username"
								type="text"
								placeholder="yourname"
								required
								class="border-zinc-600 bg-zinc-700/50 text-white placeholder:text-zinc-500"
							/>
						</div>
						<div class="space-y-2">
							<ShadLabel for="password" class="text-zinc-300">Password</ShadLabel>
							<ShadInput
								id="password"
								v-model="form.password"
								type="password"
								placeholder="••••••••"
								required
								class="border-zinc-600 bg-zinc-700/50 text-white placeholder:text-zinc-500"
							/>
						</div>
						<div class="space-y-2">
							<ShadLabel for="confirmPassword" class="text-zinc-300">Confirm Password</ShadLabel>
							<ShadInput
								id="confirmPassword"
								v-model="form.confirmPassword"
								type="password"
								placeholder="••••••••"
								required
								class="border-zinc-600 bg-zinc-700/50 text-white placeholder:text-zinc-500"
							/>
						</div>

						<div v-if="error" class="rounded-md bg-red-500/10 p-3 text-sm text-red-400">
							{{ error }}
						</div>

						<ShadButton
							type="submit"
							class="w-full bg-gradient-to-r from-amber-500 to-orange-500 font-semibold text-white hover:from-amber-600 hover:to-orange-600"
							:disabled="loading"
						>
							<Loader2 v-if="loading" class="mr-2 h-4 w-4 animate-spin" />
							{{ loading ? 'Creating Account...' : 'Create Account' }}
						</ShadButton>
					</form>
				</ShadCardContent>
				<ShadCardFooter class="flex justify-center border-t border-zinc-700 pt-6">
					<p class="text-sm text-zinc-400">
						Already have an account?
						<NuxtLink to="/auth/login" class="font-medium text-amber-400 hover:text-amber-300"> Sign in </NuxtLink>
					</p>
				</ShadCardFooter>
			</ShadCard>
		</div>
	</div>
</template>

<script setup lang="ts">
import {UserPlus, Loader2} from 'lucide-vue-next';
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

async function handleRegister() {
	error.value = '';

	if (form.value.password !== form.value.confirmPassword) {
		error.value = 'Passwords do not match';
		return;
	}

	if (form.value.password.length < 8) {
		error.value = 'Password must be at least 8 characters';
		return;
	}

	loading.value = true;

	try {
		await store.register(form.value.email, form.value.username, form.value.password);
		router.push('/');
	} catch (e: any) {
		error.value = e.message || 'Failed to create account';
	} finally {
		loading.value = false;
	}
}
</script>
