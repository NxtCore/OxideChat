export function isServerUnreachable(error: unknown): boolean {
	if (!error || typeof error !== 'object') return false;
	const e = error as Record<string, unknown>;
	const status = e['statusCode'] as number | undefined;
	// No HTTP response received (network error, ECONNREFUSED, timeout, etc.)
	if (status === undefined || status === 0) return true;
	// Server-side error
	return typeof status === 'number' && status >= 500;
}
