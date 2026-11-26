import {fileURLToPath} from 'node:url';
import type {Page} from '@playwright/test';
import {test, expect} from './support/fixtures';
import {LLM} from './support/constants';

async function pickModel(page: Page, t: (key: string) => string, displayName: string) {
	await page.getByTestId('model-selector').click();
	const search = page.getByPlaceholder(t('chat.model_selector.search_models'));
	await search.fill(displayName);
	await page.getByTestId('picker-model').filter({hasText: displayName}).first().click();
	await expect(search).toBeHidden();
}

const userMessages = (page: Page) => page.locator('[data-testid="message"][data-role="user"]');
const assistantMessages = (page: Page) => page.locator('[data-testid="message"][data-role="assistant"]');

test.describe('chat file attachments', () => {
	test.describe.configure({mode: 'serial'});

	test('a text file is read into the message, shown on it and kept after reload', async ({page, t, seed}) => {
		await page.goto('/');
		await pickModel(page, t, seed.model.displayName);
		const secret = `Launch code ${Date.now()}`;
		await page.getByTestId('composer-file-input').setInputFiles({name: 'notes.md', mimeType: 'text/markdown', buffer: Buffer.from(`# Notes\n\n${secret}\n`)});
		const chip = page.getByTestId('attached-file');
		await expect(chip).toHaveText('notes.md');

		await chip.getByRole('button', {name: t('chat.composer.remove_attachment').replace('{name}', 'notes.md')}).click();
		await expect(chip).toHaveCount(0);
		await page.getByTestId('composer-file-input').setInputFiles({name: 'notes.md', mimeType: 'text/markdown', buffer: Buffer.from(`# Notes\n\n${secret}\n`)});
		await expect(chip).toHaveText('notes.md');

		const composer = page.locator('textarea').first();
		await composer.fill('Reply with one short word.');
		await composer.press('Enter');
		await expect(page).toHaveURL(/\/chats\/[0-9a-f-]{36}/);
		await expect(assistantMessages(page)).toHaveCount(1, {timeout: 60_000});
		await expect(page.getByTestId('streaming-message')).toHaveCount(0, {timeout: 60_000});
		await expect(page.getByTestId('attached-file')).toHaveCount(0);
		await expect(userMessages(page).getByTestId('message-attachment')).toHaveText('notes.md');
		if (LLM.isMock) await expect(assistantMessages(page).first()).toContainText(secret);

		await page.reload();
		await expect(userMessages(page).getByTestId('message-attachment')).toHaveText('notes.md');
	});

	test('a PDF is read in the extraction process', async ({page, t, seed}) => {
		await page.goto('/');
		await pickModel(page, t, seed.model.displayName);
		await page.getByTestId('composer-file-input').setInputFiles(fileURLToPath(new URL('../../src/tests/fixtures/two_pages.pdf', import.meta.url)));
		await expect(page.getByTestId('attached-file')).toHaveText('two_pages.pdf');
		await expect(page.getByTestId('composer-send')).toBeEnabled();
	});

	test('sending original PDFs is off until an admin turns it on', async ({page, api}) => {
		expect((await api.get('/api/v1/base')).attachments.native_pdf).toBe(false);
		await page.goto('/settings/admin-config');
		await page.getByTestId('config-native-pdf').click();
		await page.getByTestId('settings-save-bar-save').click();
		await expect(page.getByTestId('settings-save-bar')).toBeHidden();
		try {
			expect((await api.get('/api/v1/base')).attachments.native_pdf).toBe(true);
		} finally {
			await api.patch('/api/v1/admin/config', {attachments: {native_pdf: false}});
		}
	});

	test('a file that is not supported is refused with a reason', async ({page, t, seed}) => {
		await page.goto('/');
		await pickModel(page, t, seed.model.displayName);
		await page.getByTestId('composer-file-input').setInputFiles({name: 'tool.exe', mimeType: 'application/octet-stream', buffer: Buffer.from('MZ')});
		await expect(page.getByText(t('files.error.unsupported_type'))).toBeVisible();
		await expect(page.getByTestId('attached-file')).toHaveCount(0);
		await expect(page.getByTestId('composer-send')).toBeDisabled();
	});

	test('an admin can cap file storage, which is unlimited by default', async ({page, t, api, seed}) => {
		expect((await api.get('/api/v1/base')).storage.quota_mb).toBeNull();
		await page.goto('/settings/admin-config');
		await page.getByTestId('config-storage-quota').fill('1');
		await page.getByTestId('settings-save-bar-save').click();
		await expect(page.getByTestId('settings-save-bar')).toBeHidden();
		try {
			expect((await api.get('/api/v1/base')).storage.quota_mb).toBe(1);
			await page.goto('/');
			await pickModel(page, t, seed.model.displayName);
			const megabyte = {name: 'big.txt', mimeType: 'text/plain', buffer: Buffer.from('x'.repeat(1024 * 1024))};
			await page.getByTestId('composer-file-input').setInputFiles(megabyte);
			await expect(page.getByTestId('attached-file')).toHaveCount(1);
			await page.getByTestId('composer-file-input').setInputFiles({...megabyte, name: 'more.txt'});
			await expect(page.getByText(t('files.error.quota_exceeded'))).toBeVisible();
			await expect(page.getByTestId('attached-file')).toHaveCount(1);
		} finally {
			await api.patch('/api/v1/admin/config', {storage: {quota_mb: null}});
		}
		expect((await api.get('/api/v1/base')).storage.quota_mb).toBeNull();
	});
});
