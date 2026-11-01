const mcpUrl = (admin: boolean, id = '', part = '') => `/api/v1/${admin ? 'admin/' : ''}mcp-servers${id ? '/' + id : ''}${part}`;

export const mcpServers = (admin: boolean) => useNuxtApp().$customFetch(mcpUrl(admin));

export const mcpServer = (id: string) => useNuxtApp().$customFetch(mcpUrl(true, id));

export const saveMcpServer = (admin: boolean, id: string | undefined, body: object) =>
	useNuxtApp().$customFetch(mcpUrl(admin, id), {method: id ? 'PUT' : 'POST', body});

export const deleteMcpServer = (admin: boolean, id: string) => useNuxtApp().$customFetch(mcpUrl(admin, id), {method: 'DELETE'});

export const discoverMcpServer = (admin: boolean, id: string) => useNuxtApp().$customFetch(mcpUrl(admin, id, '/discover'), {method: 'POST'});

export const syncMcpTools = (admin: boolean, id: string, tools: unknown) =>
	useNuxtApp().$customFetch(mcpUrl(admin, id, '/sync-tools'), {method: 'POST', body: {tools}});
