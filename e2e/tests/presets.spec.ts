import type {Page} from '@playwright/test';
import {test, expect} from './support/fixtures';
import {uniqueName} from './support/gateway';

async function pickModel(page: Page, t: (key: string) => string, displayName: string) {
	await openPicker(page);
	const search = page.getByPlaceholder(t('chat.model_selector.search_models'));
	await search.fill(displayName);
	await page.getByText(displayName, {exact: true}).last().click();
	await expect(search).toBeHidden();
}

async function openPicker(page: Page) {
	await page.locator('button[aria-haspopup="dialog"]').filter({has: page.locator('svg[class*="chevron-down"]')}).first().click();
}

async function pickPreset(page: Page, key: string) {
	await openPicker(page);
	await page.getByTestId('picker-presets-tab').click();
	await page.locator(`[data-testid="picker-preset"][data-preset-key="${key}"]`).click();
}

const PNG_PIXEL = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==';

const assistantMessages = (page: Page) => page.locator('[data-testid="message"][data-role="assistant"]');

function slugFor(name: string) {
	return name
		.split('/')
		.map(part => part.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, ''))
		.filter(Boolean)
		.join('/');
}

test.describe('presets', () => {
	test('a chat saved as a preset can be picked, started from a starter, restored and deleted', async ({page, t, api, seed}) => {
		const name = `e2e/${uniqueName('Reviewer')}`;
		const key = `presets/${slugFor(name)}`;
		const starter = 'Reply with one short word.';
		let presetId: string | undefined;
		try {
			await page.goto('/');
			await pickModel(page, t, seed.model.displayName);
			await page.getByTestId('preset-menu').click();
			await page.getByTestId('preset-menu-save').click();
			await expect(page).toHaveURL(/\/settings\/presets\/new$/);

			await page.getByTestId('preset-name').fill(name);
			await expect(page.getByTestId('preset-slug')).toHaveValue(slugFor(name));
			await expect(page.getByTestId('preset-slug-status')).toHaveAttribute('data-status', 'available');
			await page.getByTestId('preset-icon-input').setInputFiles({name: 'icon.png', mimeType: 'image/png', buffer: Buffer.from(PNG_PIXEL, 'base64')});
			await expect(page.getByTestId('preset-icon-preview')).toHaveAttribute('src', /\/api\/v1\/images\/[0-9a-f-]{36}$/);
			await page.getByTestId('preset-tab-instructions').click();
			await page.getByTestId('system-prompt').fill('Answer as briefly as possible.');
			await page.getByTestId('preset-tab-starters').click();
			await page.getByTestId('preset-starter-0').fill(starter);
			await page.getByTestId('settings-save-bar-save').click();

			await expect(page.getByText(t('presets.editor.saved')).first()).toBeVisible();
			await expect(page).toHaveURL(/\/settings\/presets\/[0-9a-f-]{36}$/);
			presetId = page.url().split('/').pop();
			const saved = await api.get(`/api/v1/presets/${presetId}`);
			expect(saved.key).toBe(key);
			expect(saved.base_model_id).toBe(seed.model.id);
			expect(saved.starters).toEqual([starter]);
			expect(saved.icon).toMatch(/^\/api\/v1\/images\/[0-9a-f-]{36}$/);

			await page.goto('/');
			await pickPreset(page, key);
			await expect(page.getByTestId('preset-empty-state')).toBeVisible();
			await page.getByTestId('preset-starter').filter({hasText: starter}).click();
			await expect(page).toHaveURL(/\/chats\/[0-9a-f-]{36}/);
			await expect(assistantMessages(page)).toHaveCount(1, {timeout: 60_000});
			await expect(page.getByTestId('streaming-message')).toHaveCount(0, {timeout: 60_000});
			const chatUrl = page.url();
			const chatId = chatUrl.split('/').pop();
			const messages = await api.get(`/api/v1/chats/${chatId}/messages`);
			const assistant = (messages.items ?? messages).find((message: any) => message.role === 'assistant');
			expect(assistant.model_key).toBe(key);
			expect(assistant.model_id).toBe(seed.model.id);

			await page.reload();
			await expect(page.locator('button[aria-haspopup="dialog"]').filter({hasText: name})).toBeVisible();

			await page.goto('/settings/presets');
			const row = page.locator(`[data-testid="preset-row"][data-preset-key="${key}"]`);
			await row.getByTestId('preset-row-menu').click();
			await page.getByTestId('preset-row-delete').click();
			await page.getByTestId('preset-delete-confirm').click();
			await expect(row).toHaveCount(0);
			presetId = undefined;

			await page.goto(chatUrl);
			await expect(page.getByTestId('preset-fallback-notice')).toBeVisible();
			await expect(page.locator('button[aria-haspopup="dialog"]').filter({hasText: seed.model.displayName})).toBeVisible();
			await api.delete(`/api/v1/chats/${chatId}`);
		} finally {
			if (presetId) await api.delete(`/api/v1/presets/${presetId}`);
		}
	});

	test('preset IDs cannot collide with other presets or with providers', async ({page, t, api, seed, allowErrors}) => {
		allowErrors(/409|400/);
		const slug = slugFor(uniqueName('taken'));
		const preset = await api.post('/api/v1/presets', {slug, name: 'Taken', base_model_id: seed.model.id});
		try {
			await expect(api.post('/api/v1/presets', {slug, name: 'Second', base_model_id: seed.model.id})).rejects.toThrow(/409/);
			await expect(api.post('/api/v1/admin/providers', {kind: 'OPENAI_COMPAT', name: 'Presets', base_url: 'https://example.com'})).rejects.toThrow(
				/400.*provider_name_reserved/,
			);

			await page.goto('/settings/presets/new');
			await page.getByTestId('preset-icon-change').click();
			await expect(page.getByTestId('preset-icon-dropzone')).toBeVisible();
			await page.getByTestId('preset-icon-link').fill('not a link');
			await page.getByTestId('preset-icon-link-apply').click();
			await expect(page.getByText(t('image_picker.errors.link'))).toBeVisible();
			await page.getByTestId('preset-icon-link').fill('https://example.com/icon.png');
			await page.getByTestId('preset-icon-link-apply').click();
			await expect(page.getByTestId('preset-icon-dialog')).toBeHidden();
			await expect(page.getByTestId('preset-icon-remove')).toBeVisible();
			await page.getByTestId('preset-slug').fill(slug);
			await expect(page.getByTestId('preset-slug-status')).toHaveAttribute('data-status', 'taken');
			await page.getByTestId('preset-slug').fill('Not Valid');
			await expect(page.getByTestId('preset-slug-status')).toHaveAttribute('data-status', 'invalid');
			await page.getByTestId('preset-slug').fill('team//reviewer');
			await expect(page.getByTestId('preset-slug-status')).toHaveAttribute('data-status', 'invalid');
			await page.getByTestId('preset-slug').fill(`team/${slug}`);
			await expect(page.getByTestId('preset-slug-status')).toHaveAttribute('data-status', 'available');
		} finally {
			await api.delete(`/api/v1/presets/${preset.id}`);
		}
	});

	test('the composer menu offers to edit an own preset and hides missing instructions', async ({page, api, seed}) => {
		const preset = await api.post('/api/v1/presets', {slug: slugFor(uniqueName('locked')), name: uniqueName('Locked'), base_model_id: seed.model.id, tools_locked: true});
		try {
			await page.goto('/');
			await pickPreset(page, preset.key);
			await expect(page.getByTestId('preset-empty-state')).toBeVisible();
			await page.getByTestId('preset-menu').click();
			await expect(page.getByTestId('preset-menu-edit')).toBeVisible();
			await expect(page.getByTestId('preset-menu-instructions')).toHaveCount(0);
		} finally {
			await api.delete(`/api/v1/presets/${preset.id}`);
		}
	});
});
