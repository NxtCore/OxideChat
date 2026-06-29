<template>
	<div class="flex h-screen flex-col items-center justify-center gap-4">
		<svg
			xmlns="http://www.w3.org/2000/svg"
			width="32"
			height="32"
			viewBox="0 0 24 24"
			fill="none"
			stroke="currentColor"
			stroke-width="2"
			stroke-linecap="round"
			stroke-linejoin="round"
			class="animate-spin text-primary"
		>
			<path d="M21 12a9 9 0 1 1-6.219-8.56" />
		</svg>
		<p class="text-sm text-muted-foreground">{{ statusText }}</p>
		<template v-if="store.bootState === 'server-starting'">
			<p class="text-xs text-muted-foreground">
				{{ attemptText }}
			</p>
			<button
				class="mt-1 rounded-md border border-border px-3 py-1.5 text-xs text-foreground transition-colors hover:bg-accent"
				@click="store.retryBoot()"
			>
				{{ t.retry_now }}
			</button>
		</template>
	</div>
</template>

<script setup lang="ts">
import {useMainStore} from '@/stores';

const store = useMainStore();

const STRINGS: Record<string, Record<string, string>> = {
	en: {
		booting: 'Loading…',
		server_starting: 'Server is starting up, please wait…',
		attempt: 'Attempt {n}',
		retry_now: 'Retry now',
	},
	de: {
		booting: 'Lädt…',
		server_starting: 'Server wird gestartet, bitte warten…',
		attempt: 'Versuch {n}',
		retry_now: 'Jetzt erneut versuchen',
	},
};

const lang = computed(() => {
	if (typeof navigator === 'undefined') return 'en';
	return navigator.language.slice(0, 2) === 'de' ? 'de' : 'en';
});

const t = computed(() => STRINGS[lang.value] ?? STRINGS['en']!);

const statusText = computed(() => {
	if (store.bootState === 'server-starting') return t.value.server_starting;
	return t.value.booting;
});

const attemptText = computed(() => (t.value.attempt ?? '').replace('{n}', String(store.retryCount)));
</script>
