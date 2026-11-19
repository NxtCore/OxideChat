import type {Page} from '@playwright/test';
import {test, expect} from './support/fixtures';
import {uniqueName} from './support/gateway';
import {createSyncedProvider} from './support/providers';

const slugFor = (name: string) => name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '');

async function pickArena(page: Page, key: string) {
	await page.getByTestId('model-selector').click();
	await page.getByTestId('picker-arenas-tab').click();
	await page.locator(`[data-testid="picker-arena"][data-preset-key="${key}"]`).click();
}

test.describe('arenas', () => {
	test('a blind round reveals the models after the pick and the chat continues with the winner', async ({page, t, api}) => {
		const providers: string[] = [];
		const names: Record<string, string> = {};
		let arenaId: string | undefined;
		try {
			const models: string[] = [];
			for (const label of ['Left', 'Right']) {
				const name = uniqueName(`Arena ${label}`);
				const {providerId, model} = await createSyncedProvider(api, name);
				providers.push(providerId);
				await api.patch(`/api/v1/admin/models/${model.id}`, {display_name: name});
				names[model.id] = name;
				models.push(model.id);
			}
			const arenaName = uniqueName('Blind');
			const slug = slugFor(arenaName);
			const arena = await api.post('/api/v1/presets', {
				scope: 'installation',
				mode: 'arena',
				slug,
				name: arenaName,
				base_model_id: models[0],
				levels: models.map((model_id, index) => ({key: `m${index + 1}`, model_id})),
			});
			arenaId = arena.id;
			const key = `presets/${slug}`;

			await page.goto('/settings/presets?tab=organization');
			await expect(page.locator(`[data-testid="preset-row"][data-preset-key="${key}"]`)).toHaveCount(0);
			await page.goto('/settings/arenas');
			const row = page.locator(`[data-testid="arena-row"][data-preset-key="${key}"]`);
			await expect(row).toContainText(t('presets.arena.picker_subtitle', {count: 2}));

			await row.locator('a').click();
			await page.getByTestId('preset-tab-options').click();
			await page.getByTestId('arena-option-stream_responses').click();
			await page.getByTestId('settings-save-bar-save').click();
			await expect(page.getByText(t('presets.editor.saved')).first()).toBeVisible();
			expect((await api.get(`/api/v1/presets/${arena.id}`)).arena).toEqual({stream_responses: false, show_reasoning: false});

			await page.goto('/');
			await pickArena(page, key);
			await expect(page.getByTestId('model-selector-arena')).toBeVisible();
			const emptyState = page.getByTestId('preset-empty-state').locator('..');
			await expect(emptyState).toContainText(t('presets.arena.picker_subtitle', {count: 2}));
			for (const name of Object.values(names)) await expect(emptyState).not.toContainText(name);
			await page.getByTestId('chat-composer').locator('textarea').fill('Reply with one short word.');
			await page.getByTestId('composer-send').click();

			const round = page.getByTestId('arena-round');
			await expect(round.getByTestId('arena-pick-1')).toBeEnabled({timeout: 60_000});
			await expect(round.getByTestId('arena-candidate-model')).toHaveCount(0);
			await expect(page.getByTestId('arena-pick-to-continue')).toBeVisible();
			await expect(page.getByTestId('composer-send')).toBeDisabled();

			await round.getByTestId('arena-pick-1').click();
			await expect(page.getByTestId('arena-round')).toHaveCount(0);
			await expect(page.getByTestId('arena-pick-to-continue')).toHaveCount(0);

			const chatId = page.url().split('/').pop()!;
			const chat = await api.get(`/api/v1/chats/${chatId}`);
			const winner = chat.messages.at(-1);
			expect(winner.arena_round_id).toBeTruthy();
			expect(chat.pending_arena_round).toBeNull();
			await expect(page.getByTestId('model-selector-label')).toHaveText(names[winner.model_id]);

			await page.getByTestId('chat-composer').locator('textarea').fill('Reply with one more short word.');
			await page.getByTestId('composer-send').click();
			const assistant = page.locator('[data-testid="message"][data-role="assistant"]');
			await expect(assistant).toHaveCount(2, {timeout: 60_000});
			await expect(page.getByTestId('arena-round')).toHaveCount(0);

			await assistant.first().hover();
			await assistant.first().getByTestId('message-action-arena').click();
			const dialog = page.getByTestId('arena-compare-dialog');
			await expect(dialog.getByTestId('arena-compare-answer')).toHaveCount(2);
			await expect(dialog.getByTestId('arena-compare-picked')).toHaveCount(1);
			for (const name of Object.values(names)) await expect(dialog).toContainText(name);
			await page.keyboard.press('Escape');

			await page.goto(`/settings/arenas/${arena.id}`);
			await page.getByTestId('preset-tab-results').click();
			const rows = page.getByTestId('arena-results-row');
			await expect(rows).toHaveCount(2);
			await expect(rows.first()).toContainText(names[winner.model_id]);
			await expect(rows.first().getByTestId('arena-results-wins')).toHaveText('1');
			await expect(rows.first().getByTestId('arena-results-rounds')).toHaveText('1');
		} finally {
			if (arenaId) await api.delete(`/api/v1/presets/${arenaId}`);
			for (const providerId of providers) await api.delete(`/api/v1/admin/providers/${providerId}`);
		}
	});
});
