<template>
	<div class="flex min-h-screen items-center justify-center bg-gradient-to-br from-zinc-900 via-zinc-800 to-zinc-900">
		<div class="w-full max-w-md space-y-8 px-4">
			<!-- Login Card -->
			<ShadCard class="border-zinc-700 bg-zinc-800/50 backdrop-blur">
				<ShadCardHeader class="text-center">
					<div class="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-full bg-gradient-to-br from-violet-400 to-fuchsia-500">
						<LogIn class="h-8 w-8 text-white" />
					</div>
					<ShadCardTitle class="text-2xl font-bold text-white">Welcome Back</ShadCardTitle>
					<ShadCardDescription class="text-zinc-400"> Sign in to continue to OxideChat </ShadCardDescription>
				</ShadCardHeader>
				<ShadCardContent>
					<form @submit.prevent="handleLogin" class="space-y-4">
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

						<div v-if="error" class="rounded-md bg-red-500/10 p-3 text-sm text-red-400">
							{{ error }}
						</div>

						<ShadButton
							type="submit"
							class="w-full bg-gradient-to-r from-violet-500 to-fuchsia-500 font-semibold text-white hover:from-violet-600 hover:to-fuchsia-600"
							:disabled="loading"
						>
							<Loader2 v-if="loading" class="mr-2 h-4 w-4 animate-spin" />
							{{ loading ? 'Signing in...' : 'Sign In' }}
						</ShadButton>
					</form>
				</ShadCardContent>
				<ShadCardFooter class="flex justify-center border-t border-zinc-700 pt-6">
					<p class="text-sm text-zinc-400">
						Don't have an account?
						<NuxtLink to="/auth/register" class="font-medium text-violet-400 hover:text-violet-300"> Sign up </NuxtLink>
					</p>
				</ShadCardFooter>
			</ShadCard>
		</div>
	</div>
</template>

<script setup lang="ts">
import {LogIn, Loader2} from 'lucide-vue-next';
import {useMainStore} from '@/stores';

const store = useMainStore();
const router = useRouter();

const form = ref({
	email: '',
	password: '',
});

const loading = ref(false);
const error = ref('');

async function handleLogin() {
	error.value = '';
	loading.value = true;

	try {
		await store.login(form.value.email, form.value.password);
		router.push('/');
	} catch (e: any) {
		error.value = e.message || 'Invalid email or password';
	} finally {
		loading.value = false;
	}
}
</script>
