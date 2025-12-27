<template>
	<div class="flex h-full flex-col">
		<!-- Main chat area placeholder -->
		<div class="flex flex-1 items-center justify-center">
			<div class="text-center">
				<div class="mx-auto mb-6 flex h-20 w-20 items-center justify-center rounded-full bg-primary">
					<MessageSquare class="h-10 w-10 text-primary-foreground" />
				</div>
				<h1 class="text-2xl font-bold text-foreground">{{ store.getTranslation('welcome.title') }}</h1>
				<p class="mt-2 text-muted-foreground">{{ store.getTranslation('welcome.description') }}</p>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import {MessageSquare} from 'lucide-vue-next';
import {useMainStore} from '@/stores';

const store = useMainStore();

// Page is protected by auth middleware
</script>
