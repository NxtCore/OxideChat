import type {Page} from '@playwright/test';
import {test, expect} from './support/fixtures';
import {uniqueName} from './support/gateway';
import {MOCK_LLM_KEY, MOCK_LLM_URL} from './support/constants';

async function openPicker(page: Page) {
	await page.locator('button[aria-haspopup="dialog"]').filter({has: page.locator('svg[class*="chevron-down"]')}).first().click();
}

async function scrollToEnd(page: Page) {
	await page.getByTestId('picker-model').last().scrollIntoViewIfNeeded();
}

test.describe('model picker', () => {
	test('scrolling loads more models, also after reopening', async ({page, api, t}) => {
		const name = uniqueName('Bulk');
		const provider = await api.post('/api/v1/admin/providers', {kind: 'OPENAI_COMPAT', name, base_url: `${MOCK_LLM_URL}/bulk`, api_key: MOCK_LLM_KEY});
		try {
			const sync = await api.post(`/api/v1/admin/providers/${provider.id}/sync`);
			expect(sync.success).toBe(true);
			await page.goto('/');
			await openPicker(page);
			await page.locator(`button[title="${name}"]`).click();
			const rows = page.getByTestId('picker-model');
			await expect(rows).toHaveCount(50);
			await scrollToEnd(page);
			await expect(rows).toHaveCount(100);
			await scrollToEnd(page);
			await expect(rows).toHaveCount(120);
			await page.keyboard.press('Escape');

			await openPicker(page);
			await expect(rows).toHaveCount(120);
			await page.getByPlaceholder(t('chat.model_selector.search_models')).fill('bulk-model');
			await expect(rows).toHaveCount(50);
			await scrollToEnd(page);
			await expect(rows).toHaveCount(100);
		} finally {
			await api.delete(`/api/v1/admin/providers/${provider.id}`);
		}
	});
});
