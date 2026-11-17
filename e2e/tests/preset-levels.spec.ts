import type {Page} from '@playwright/test';
import {test, expect} from './support/fixtures';
import {uniqueName} from './support/gateway';

const slugFor = (name: string) => name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '');

const assistantMessages = (page: Page) => page.locator('[data-testid="message"][data-role="assistant"]');

async function pickPreset(page: Page, key: string) {
	await page.getByTestId('model-selector').click();
	await page.getByTestId('picker-presets-tab').click();
	await page.locator(`[data-testid="picker-preset"][data-preset-key="${key}"]`).click();
}

test.describe('preset levels', () => {
	test('an editor turns a preset into a slider with levels', async ({page, t, api, seed}) => {
		const name = uniqueName('Slider');
		const preset = await api.post('/api/v1/presets', {slug: slugFor(name), name, base_model_id: seed.model.id});
		try {
			await page.goto(`/settings/presets/${preset.id}`);
			await page.getByTestId('preset-tab-levels').click();
			await expect(page.getByTestId('preset-levels-empty')).toBeVisible();
			await page.getByTestId('preset-levels-enable').click();

			const levels = page.getByTestId('preset-level');
			await expect(levels).toHaveCount(2);
			await expect(levels.nth(0)).toHaveAttribute('data-level', 'low');
			await expect(levels.nth(1)).toHaveAttribute('data-level', 'high');
			await expect(levels.nth(0).getByTestId('preset-level-label')).toHaveAttribute('placeholder', t('presets.level.low'));

			await page.getByTestId('preset-levels-add').click();
			await expect(levels).toHaveCount(3);
			await expect(levels.nth(2)).toHaveAttribute('data-level', 'medium');
			await levels.nth(2).getByTestId('preset-level-remove').click();
			await expect(levels).toHaveCount(2);

			await levels.nth(1).getByTestId('preset-level-default').click();
			await levels.nth(1).getByTestId('preset-level-label').fill('Deep thinking');
			await levels.nth(1).getByTestId('preset-level-hint').fill('Hard problems');
			await levels.nth(1).getByTestId('preset-level-key').fill('deep');

			await page.getByTestId('preset-tab-general').click();
			await expect(page.getByTestId('preset-base-model-hint')).toHaveText(t('presets.editor.model_from_levels'));

			await page.getByTestId('settings-save-bar-save').click();
			await expect(page.getByText(t('presets.editor.saved')).first()).toBeVisible();

			const saved = await api.get(`/api/v1/presets/${preset.id}`);
			expect(saved.levels.map((level: any) => [level.key, level.label, level.hint, level.model_id])).toEqual([
				['low', null, null, seed.model.id],
				['deep', 'Deep thinking', 'Hard problems', seed.model.id],
			]);
			expect(saved.default_level).toBe('deep');
			expect(saved.base_model_id).toBe(seed.model.id);

			await page.reload();
			await page.getByTestId('preset-tab-levels').click();
			await expect(page.getByTestId('preset-level')).toHaveCount(2);
			await page.getByTestId('preset-levels-disable').click();
			await page.getByTestId('settings-save-bar-save').click();
			await expect(page.getByText(t('presets.editor.saved')).first()).toBeVisible();
			expect((await api.get(`/api/v1/presets/${preset.id}`)).levels).toEqual([]);
		} finally {
			await api.delete(`/api/v1/presets/${preset.id}`);
		}
	});

	test('the composer level picks the model of each message and is restored with the chat', async ({page, t, api, seed}) => {
		const name = uniqueName('Gemini');
		const slug = slugFor(name);
		const preset = await api.post('/api/v1/presets', {
			slug,
			name,
			base_model_id: seed.model.id,
			levels: [
				{key: 'low', model_id: seed.model.id, hint: 'Quick answers'},
				{key: 'max', label: 'Complex', model_id: seed.model.id, hint: 'Hard problems'},
			],
		});
		const key = `presets/${slug}`;
		let chatId: string | undefined;
		try {
			await page.goto('/');
			await pickPreset(page, key);
			await expect(page.getByTestId('model-selector')).toHaveAttribute('title', name);
			const level = page.getByTestId('composer-level');
			await expect(level).toHaveAttribute('data-level', 'low');
			await expect(level).toContainText(t('presets.level.low'));
			await expect(page.getByTestId('composer-chip-reasoning')).toHaveCount(0);

			await level.click();
			const panel = page.getByTestId('composer-level-panel');
			await expect(panel.getByTestId('composer-level-title')).toHaveText(t('presets.level.low'));
			await expect(panel.getByTestId('composer-level-hint')).toHaveText('Quick answers');
			await expect(panel.getByTestId('composer-level-reset')).toHaveCount(0);
			await expect(panel.locator('[data-testid="composer-level-option"][aria-checked="true"]')).toHaveAttribute('data-level', 'low');

			await panel.locator('[data-testid="composer-level-option"][data-level="max"]').click();
			await expect(level).toHaveAttribute('data-level', 'max');
			await expect(panel.getByTestId('composer-level-title')).toHaveText('Complex');
			await expect(panel.getByTestId('composer-level-hint')).toHaveText('Hard problems');

			await panel.getByTestId('composer-level-reset').click();
			await expect(level).toHaveAttribute('data-level', 'low');
			await panel.locator('[data-testid="composer-level-option"][aria-checked="true"]').focus();
			await page.keyboard.press('ArrowRight');
			await expect(level).toHaveAttribute('data-level', 'max');
			await expect(level).toContainText('Complex');
			await page.keyboard.press('Escape');
			await expect(panel).toBeHidden();

			await page.getByTestId('chat-composer').locator('textarea').fill('Reply with one short word.');
			await page.getByTestId('composer-send').click();
			await expect(page).toHaveURL(/\/chats\/[0-9a-f-]{36}/);
			await expect(assistantMessages(page)).toHaveCount(1, {timeout: 60_000});
			await expect(page.getByTestId('streaming-message')).toHaveCount(0, {timeout: 60_000});
			chatId = page.url().split('/').pop();

			const storedAssistant = async () => {
				const messages = await api.get(`/api/v1/chats/${chatId}/messages`);
				return (messages.items ?? messages).find((message: any) => message.role === 'assistant');
			};
			await expect.poll(storedAssistant).toBeTruthy();
			const assistant = await storedAssistant();
			expect(assistant.model_key).toBe(`${key}:max`);
			expect(assistant.model_id).toBe(seed.model.id);

			await page.reload();
			await expect(page.getByTestId('model-selector')).toHaveAttribute('title', name);
			await expect(page.getByTestId('composer-level')).toHaveAttribute('data-level', 'max');
		} finally {
			if (chatId) await api.delete(`/api/v1/chats/${chatId}`);
			await api.delete(`/api/v1/presets/${preset.id}`);
		}
	});

	test('levels added or removed in the editor reach the composer without a reload', async ({page, t, api, seed}) => {
		const name = uniqueName('Live');
		const preset = await api.post('/api/v1/presets', {slug: slugFor(name), name, base_model_id: seed.model.id});
		try {
			await page.goto('/');
			await pickPreset(page, preset.key);
			await expect(page.getByTestId('composer-level')).toHaveCount(0);

			await page.getByTestId('model-selector').click();
			await page.getByTestId('picker-presets-tab').click();
			await page.locator(`[data-testid="picker-preset"][data-preset-key="${preset.key}"]`).getByTestId('picker-preset-edit').click();
			await expect(page).toHaveURL(new RegExp(`/settings/presets/${preset.id}$`));
			await page.getByTestId('preset-tab-levels').click();
			await page.getByTestId('preset-levels-enable').click();
			await page.getByTestId('settings-save-bar-save').click();
			await expect(page.getByText(t('presets.editor.saved')).first()).toBeVisible();

			await page.goBack();
			await expect(page).not.toHaveURL(/\/settings\//);
			await expect(page.getByTestId('model-selector')).toHaveAttribute('title', name);
			await expect(page.getByTestId('composer-level')).toHaveAttribute('data-level', 'low');

			await page.getByTestId('model-selector').click();
			await page.getByTestId('picker-presets-tab').click();
			await page.locator(`[data-testid="picker-preset"][data-preset-key="${preset.key}"]`).getByTestId('picker-preset-edit').click();
			await page.getByTestId('preset-tab-levels').click();
			await page.getByTestId('preset-levels-disable').click();
			await page.getByTestId('settings-save-bar-save').click();
			await expect(page.getByText(t('presets.editor.saved')).first()).toBeVisible();

			await page.goBack();
			await expect(page.getByTestId('model-selector')).toHaveAttribute('title', name);
			await expect(page.getByTestId('composer-level')).toHaveCount(0);
		} finally {
			await api.delete(`/api/v1/presets/${preset.id}`);
		}
	});
});
