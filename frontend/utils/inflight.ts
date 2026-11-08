const pending = new Map<string, Promise<void>>();

export function inflight(key: string, load: () => Promise<unknown>): Promise<void> {
	let request = pending.get(key);
	if (!request) {
		request = load()
			.then(() => undefined)
			.finally(() => pending.delete(key));
		pending.set(key, request);
	}
	return request;
}
