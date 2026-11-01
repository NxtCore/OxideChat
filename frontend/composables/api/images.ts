export const imageUrl = (id: string, base = '') => `${base}/api/v1/images/${encodeURIComponent(id)}`;

export const postImage = (dataUri: string) =>
	fetch(`${useRuntimeConfig().public.apiBase || ''}/api/v1/images`, {
		method: 'POST',
		headers: {'Content-Type': 'application/json'},
		credentials: 'include',
		body: JSON.stringify({data_uri: dataUri}),
	});
