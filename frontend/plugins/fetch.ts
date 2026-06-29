import {useMainStore} from '~/stores';

export default defineNuxtPlugin(() => {
    const config = useRuntimeConfig();
    const cookie = process.server ? useRequestHeaders(['cookie']).cookie : undefined;

    const customFetch = $fetch.create({
        baseURL: process.server ? config.baseURL : config.public.baseURL,
        credentials: 'include',
        headers: {
            cookie: cookie || '',
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
