import type {ProviderKind} from '~/types/generated/ProviderKind';

export type ProviderAllowlists = {[key in ProviderKind]?: string[]};

const GATEWAY_NAMES: {[key in ProviderKind]?: string} = {OPENROUTER: 'OpenRouter', CORTECS: 'Cortecs', MISTRAL: 'Mistral'};

export function gatewayName(kind: ProviderKind): string {
	return GATEWAY_NAMES[kind] ?? kind.charAt(0) + kind.slice(1).toLowerCase().replaceAll('_', ' ');
}

export function copyAllowlists(lists: ProviderAllowlists | null | undefined): ProviderAllowlists {
	const entries = Object.entries(lists ?? {}) as [ProviderKind, string[]][];
	return Object.fromEntries(entries.sort(([a], [b]) => a.localeCompare(b)).map(([kind, slugs]) => [kind, [...slugs].sort()]));
}

export function withAllowlist(lists: ProviderAllowlists, kind: ProviderKind, slugs: string[] | null): ProviderAllowlists {
	const next: ProviderAllowlists = {...lists};
	if (slugs === null) delete next[kind];
	else next[kind] = slugs;
	return copyAllowlists(next);
}

export function emptyAllowlistError(lists: ProviderAllowlists): string | null {
	return Object.values(lists).some(slugs => !slugs?.length) ? 'settings.provider_allowlist.empty' : null;
}
