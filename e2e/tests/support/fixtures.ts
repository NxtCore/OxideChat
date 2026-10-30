import {readFileSync} from 'node:fs';
import {test as base, expect, type APIRequestContext, type Page} from '@playwright/test';
import {AUTH_FILES} from './constants';

export type Seed = {
	adminId: string;
	memberId: string;
	providerId: string;
	teamId: string;
	teamName: string;
	model: import('./providers').SyncedModel;
	pricing: import('./cost').Pricing;
};

type Translator = (key: string, args?: Record<string, string | number>) => string;

type Fixtures = {
	t: Translator;
	api: Api;
	seed: Seed;
	allowErrors: (...patterns: RegExp[]) => void;
};

type WorkerFixtures = {translations: Record<string, unknown>};

export class Api {
	constructor(readonly request: APIRequestContext) {}

	async call<T = any>(method: string, url: string, data?: unknown): Promise<T> {
		const response = await this.request.fetch(url, {method, data});
		const text = await response.text();
		if (!response.ok()) throw new Error(`${method} ${url} -> ${response.status()}: ${text}`);
		return (text ? JSON.parse(text) : undefined) as T;
	}

	get<T = any>(url: string) {
		return this.call<T>('GET', url);
	}

	post<T = any>(url: string, data?: unknown) {
		return this.call<T>('POST', url, data ?? {});
	}

	put<T = any>(url: string, data?: unknown) {
		return this.call<T>('PUT', url, data ?? {});
	}

	patch<T = any>(url: string, data?: unknown) {
		return this.call<T>('PATCH', url, data ?? {});
	}

	delete<T = any>(url: string) {
		return this.call<T>('DELETE', url);
	}
}

export function translate(translations: Record<string, unknown>, key: string, args: Record<string, string | number> = {}): string {
	let current: unknown = translations;
	for (const part of key.split('.')) {
		current = (current as Record<string, unknown> | undefined)?.[part];
	}
	if (typeof current !== 'string') throw new Error(`Missing translation for "${key}"`);
	return current.replace(/{(\w+)}/g, (match, name) => String(args[name] ?? match));
}

export async function untranslatedKeys(page: Page): Promise<string[]> {
	return page.evaluate(() => {
		const pattern = /^[a-z][a-z0-9_]*(\.[a-z0-9_]+){1,}$/;
		const found = new Set<string>();
		const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
		while (walker.nextNode()) {
			const text = walker.currentNode.textContent?.trim() ?? '';
			const parent = walker.currentNode.parentElement;
			if (!parent || parent.closest('code, pre, script, style, [data-allow-key]')) continue;
			if (pattern.test(text) && !/\.(com|test|ts|js|json|md)$/.test(text)) found.add(text);
		}
		for (const element of document.querySelectorAll('[placeholder], [title], [aria-label]')) {
			for (const name of ['placeholder', 'title', 'aria-label']) {
				const value = element.getAttribute(name)?.trim();
				if (value && pattern.test(value)) found.add(value);
			}
		}
		return [...found];
	});
}

export const test = base.extend<Fixtures, WorkerFixtures>({
	translations: [
		async ({playwright}, use) => {
			const request = await playwright.request.newContext({baseURL: `http://localhost:${process.env.E2E_PORT ?? 3100}`});
			const base = await (await request.get('/api/v1/base')).json();
			await request.dispose();
			await use(base.i18n.en);
		},
		{scope: 'worker'},
	],
	t: async ({translations}, use) => {
		await use((key, args) => translate(translations, key, args));
	},
	api: async ({page}, use) => {
		await use(new Api(page.request));
	},
	seed: async ({}, use) => {
		await use(JSON.parse(readFileSync(AUTH_FILES.seed, 'utf8')) as Seed);
	},
	allowErrors: [
		async ({page}, use, testInfo) => {
			const allowed: RegExp[] = [];
			const problems: string[] = [];
			const isAllowed = (message: string) => allowed.some(pattern => pattern.test(message));
			page.on('pageerror', error => {
				if (!isAllowed(error.message)) problems.push(`pageerror: ${error.message}`);
			});
			page.on('console', message => {
				if (message.type() === 'error' && !message.text().startsWith('Failed to load resource') && !isAllowed(message.text())) problems.push(`console.error: ${message.text()}`);
			});
			page.on('response', response => {
				const url = response.url();
				if (response.status() >= 500 && !isAllowed(`${response.status()} ${url}`)) problems.push(`HTTP ${response.status()} ${response.request().method()} ${url}`);
			});
			await use((...patterns) => allowed.push(...patterns));
			if (problems.length) {
				await testInfo.attach('browser-errors', {body: problems.join('\n'), contentType: 'text/plain'});
			}
			expect(problems, 'browser errors and failed API calls during the test').toEqual([]);
		},
		{auto: true},
	],
});

export {expect};
