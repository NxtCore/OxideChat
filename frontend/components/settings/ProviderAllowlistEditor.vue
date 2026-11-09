<template>
	<div class="space-y-2" :data-testid="testId">
		<p v-if="loaded && !gateways.length" class="text-xs text-muted-foreground" :data-testid="`${testId}-no-gateways`">
			{{ store.getTranslation('settings.provider_allowlist.no_gateways') }}
		</p>
		<div v-for="gateway in gateways" :key="gateway.kind" class="rounded-md border border-border">
			<button
				type="button"
				class="flex w-full items-center gap-3 px-3 py-2 text-left hover:bg-accent/50"
				:aria-expanded="isOpen(gateway.kind)"
				:data-testid="`${testId}-gateway-${slug(gateway.kind)}`"
				@click="setOpen(gateway.kind, !isOpen(gateway.kind))"
			>
				<ChevronRight class="h-4 w-4 shrink-0 text-muted-foreground transition-transform" :class="{'rotate-90': isOpen(gateway.kind)}" />
				<span class="flex-1 text-sm font-medium">{{ gatewayName(gateway.kind) }}</span>
				<ShadBadge :variant="modelValue[gateway.kind] ? 'default' : 'secondary'" :data-testid="`${testId}-gateway-${slug(gateway.kind)}-status`">
					{{ status(gateway.kind) }}
				</ShadBadge>
			</button>
			<div v-if="isOpen(gateway.kind)" class="space-y-3 border-t border-border p-3">
				<div class="flex items-start justify-between gap-3 text-xs text-muted-foreground">
					<div class="space-y-1">
						<p :data-testid="`${testId}-${slug(gateway.kind)}-synced`">
							{{
								gateway.synced_at
									? store.getTranslation('settings.provider_allowlist.synced_at', {time: formatDateTime(gateway.synced_at)})
									: store.getTranslation('settings.provider_allowlist.not_synced')
							}}
						</p>
						<p v-if="failed.includes(gateway.kind)" class="text-destructive">
							{{ store.getTranslation('settings.provider_allowlist.sync_failed', {gateway: gatewayName(gateway.kind)}) }}
						</p>
					</div>
					<ShadButton
						v-if="canSync && gateway.provider_id"
						variant="outline"
						size="sm"
						class="shrink-0"
						:disabled="syncing || disabled"
						:data-testid="`${testId}-${slug(gateway.kind)}-sync`"
						@click="sync"
					>
						<RefreshCw class="h-3.5 w-3.5" :class="{'animate-spin': syncing}" />
						{{ store.getTranslation('settings.provider_allowlist.sync') }}
					</ShadButton>
				</div>
				<UpstreamProviderPicker
					:model-value="modelValue[gateway.kind] ?? null"
					:providers="gateway.providers"
					:within="within?.[gateway.kind] ?? null"
					:disabled="disabled"
					:test-id="`${testId}-${slug(gateway.kind)}`"
					@update:model-value="emit('update:modelValue', withAllowlist(modelValue, gateway.kind, $event))"
				/>
			</div>
		</div>
	</div>
</template>

<script setup lang="ts">
import {ChevronRight, RefreshCw} from 'lucide-vue-next';
import {useMainStore} from '@/stores';
import type {ProviderKind} from '~/types/generated/ProviderKind';
import type {UpstreamGatewayResponse} from '~/types/generated/UpstreamGatewayResponse';
import type {UpstreamSyncResponse} from '~/types/generated/UpstreamSyncResponse';
import UpstreamProviderPicker from '~/components/settings/UpstreamProviderPicker.vue';

const props = defineProps<{modelValue: ProviderAllowlists; within?: ProviderAllowlists | null; disabled?: boolean; canSync?: boolean; testId: string}>();
const emit = defineEmits<{'update:modelValue': [value: ProviderAllowlists]}>();

const store = useMainStore();
const loadedGateways = useState<UpstreamGatewayResponse[] | null>('upstream-gateways', () => null);
const opened = ref<ProviderKind[]>([]);
const syncing = ref(false);
const failed = ref<ProviderKind[]>([]);

const loaded = computed(() => loadedGateways.value != null);

const gateways = computed<UpstreamGatewayResponse[]>(() => {
	const list = [...(loadedGateways.value ?? [])];
	const kinds = new Set([...Object.keys(props.modelValue), ...Object.keys(props.within ?? {})] as ProviderKind[]);
	for (const kind of kinds) {
		if (!list.some(gateway => gateway.kind === kind)) list.push({kind, provider_id: null, synced_at: null, providers: []});
	}
	return list.sort((a, b) => gatewayName(a.kind).localeCompare(gatewayName(b.kind)));
});

function slug(kind: ProviderKind) {
	return kind.toLowerCase();
}

function isOpen(kind: ProviderKind) {
	return opened.value.includes(kind) || gateways.value.length === 1;
}

function setOpen(kind: ProviderKind, open: boolean) {
	opened.value = open ? [...opened.value, kind] : opened.value.filter(value => value !== kind);
}

function status(kind: ProviderKind) {
	const allowed = props.modelValue[kind];
	if (!allowed) return store.getTranslation('settings.provider_allowlist.all_allowed');
	return store.getTranslation('settings.provider_allowlist.count_allowed', {count: allowed.length});
}

async function sync() {
	syncing.value = true;
	try {
		const result = await syncUpstreamProviders<UpstreamSyncResponse>();
		loadedGateways.value = result.gateways;
		failed.value = result.failed;
	} catch (error) {
		store.toast(errorMessage(error, store.getTranslation('settings.provider_allowlist.sync_error')), {type: 'error'});
	} finally {
		syncing.value = false;
	}
}

onMounted(() =>
	inflight('upstream-gateways', async () => {
		if (loadedGateways.value) return;
		try {
			loadedGateways.value = await upstreamProviders<UpstreamGatewayResponse>();
		} catch (error) {
			console.error('Failed to load upstream providers:', error);
			loadedGateways.value = [];
		}
	}),
);
</script>
