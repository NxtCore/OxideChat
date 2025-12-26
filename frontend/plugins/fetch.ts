export default defineNuxtPlugin(nuxtApp => {
    const config = useRuntimeConfig();
    // On the server, useRequestHeaders to extract the "cookie" header:
    const cookie = process.server ? useRequestHeaders(['cookie']).cookie : undefined;

    // Create a custom fetch instance with credentials and forwarded cookies
    const customFetch = $fetch.create({
        baseURL: process.server ? config.baseURL : config.public.baseURL,
        credentials: 'include',
        headers: {
            cookie: cookie || '',
        },
    });

    return {
        provide: {
            customFetch,
        },
    };
});
