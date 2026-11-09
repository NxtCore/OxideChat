import {test, expect} from './support/fixtures';

const BUDGET_STATUS = {
	budgets: [{amount: '20', spent: '7.5', remaining: '12.5'}],
	decision: 'allow',
	blocked_model_ids: [],
};

test.describe('sidebar budget', () => {
	test('budget can be pinned to the sidebar and stays pinned after reload', async ({page, t}) => {
		await page.route('**/api/v1/me/budget', route => route.fulfill({json: BUDGET_STATUS}));
		await page.goto('/');
		await page.evaluate(() => localStorage.removeItem('oxide-budget-pinned'));
		await page.reload();

		const pinned = page.getByTestId('pinned-budget');
		const remaining = t('sidebar.budget.remaining', {amount: '$12.50'});
		await expect(pinned).toBeHidden();

		await page.getByTestId('user-menu-trigger').click();
		await expect(page.getByRole('menu').getByText(remaining)).toBeVisible();
		await page.getByRole('menu').getByTestId('budget-pin-toggle').click();

		await expect(pinned).toBeVisible();
		await expect(pinned.getByText(remaining)).toBeVisible();
		await expect(page.getByRole('menu').getByText(remaining)).toBeHidden();

		await page.keyboard.press('Escape');
		await page.reload();
		await expect(pinned).toBeVisible();

		await pinned.getByTestId('budget-pin-toggle').click();
		await expect(pinned).toBeHidden();
	});
});
