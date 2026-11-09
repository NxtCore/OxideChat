import {useMainStore} from '~/stores';
import {useModelStore} from '~/stores/modelStore';
import {usePreferencesStore} from '~/stores/preferencesStore';
import {useBudgetStore} from '~/stores/budgetStore';
import {useDeveloperStore} from '~/stores/developerStore';

const STALE_AFTER_WRITE: {path: RegExp; markStale: () => void}[] = [
    {path: /^\/api\/v1\/admin\/(models|image-models|providers|teams|users|config)(\/|$)/, markStale: () => (useModelStore().stale = true)},
    {path: /^\/api\/v1\/(admin\/)?(tools|mcp-servers)(\/|$)/, markStale: () => (usePreferencesStore().toolsStale = true)},
    {path: /^\/api\/v1\/admin\/(budgets|teams|users)(\/|$)/, markStale: () => (useBudgetStore().myStatusStale = true)},
    {path: /^\/api\/v1\/admin\/(models|image-models|providers|teams|users|gateway)(\/|$)/, markStale: () => useDeveloperStore().$patch({loaded: false, modelsLoaded: false})},
    {path: /^\/api\/v1\/admin\/providers(\/|$)/, markStale: () => clearNuxtState('upstream-gateways')},
];

function markStaleAfterWrite(request: RequestInfo, method: string | undefined) {
    if (!method || method.toUpperCase() === 'GET') return;
    const path = new URL(typeof request === 'string' ? request : request.url, 'http://localhost').pathname;
    for (const rule of STALE_AFTER_WRITE) {
        if (rule.path.test(path)) rule.markStale();
    }
}

export default defineNuxtPlugin(() => {
    const config = useRuntimeConfig();
    const cookie = process.server ? useRequestHeaders(['cookie']).cookie : undefined;

    const customFetch = $fetch.create({
        baseURL: process.server ? config.baseURL : config.public.baseURL,
        credentials: 'include',
        headers: {
            cookie: cookie || '',
        },
        onResponse({request, options, response}) {
            if (process.client && response.ok) markStaleAfterWrite(request, options.method);
        },
        onResponseError({response}) {
            if (!process.client) return;
            if (response.status >= 500) {
                const store = useMainStore();
                if (store.bootState === 'booting') {
                    store.bootState = 'server-starting';
                }
            }
        },
        onRequestError() {
            if (!process.client) return;
            const store = useMainStore();
            if (store.bootState === 'booting') {
                store.bootState = 'server-starting';
            }
        },
    });

    return {
        provide: {
            customFetch,
        },
    };
});
