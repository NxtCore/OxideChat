import {test, expect} from './support/fixtures';
import {AUTH_FILES} from './support/constants';

const NAME = 'E2E Support desk';
const RENAMED = 'E2E Helpdesk';

type Role = {id: string; name: string; is_builtin: boolean; permissions: string[]; user_count: number};

test.describe.configure({mode: 'serial'});

test.describe('roles and permissions', () => {
	test('built-in roles keep their name and the last role that can manage roles keeps that permission', async ({page, api, t, allowErrors}) => {
		allowErrors(/409/);
		await page.goto('/settings/roles');
		const adminRow = page.locator('[data-testid="role-row"][data-role-name="admin"]');
		await expect(adminRow).toContainText(t('roles.builtin'));
		await expect(adminRow.getByTestId('role-delete')).toHaveCount(0);
		await adminRow.getByTestId('role-edit').click();
		await expect(page.getByText(t('roles.builtin_hint'))).toBeVisible();
		await expect(page.getByTestId('role-name')).toBeDisabled();
		await page.getByTestId('role-permission-admin.*').click();
		await page.getByTestId('role-save').click();
		await expect(page.getByText(t('roles.errors.lock_out'))).toBeVisible();

		const admin = (await api.get<Role[]>('/api/v1/admin/roles')).find(role => role.name === 'admin');
		expect(admin?.permissions).toContain('admin.*');
	});

	test('an admin creates a role, a member gets its permissions, and losing it takes them away', async ({page, api, seed, browser, baseURL, t}) => {
		await page.goto('/settings/roles');
		await page.getByTestId('role-create').click();
		await page.getByTestId('role-name').fill(NAME);
		await expect(page.getByTestId('role-group-admin_users')).toContainText(t('roles.group.admin_users'));
		await page.getByTestId('role-permission-admin.users.view').click();
		await page.getByTestId('role-permission-admin.roles.view').click();
		await page.getByTestId('role-save').click();
		const row = page.locator(`[data-testid="role-row"][data-role-name="${NAME}"]`);
		await expect(row).toContainText(t('roles.permission_count', {count: 2}));

		const created = (await api.get<Role[]>('/api/v1/admin/roles')).find(role => role.name === NAME);
		expect(created).toMatchObject({is_builtin: false, permissions: ['admin.roles.view', 'admin.users.view'], user_count: 0});

		const before = await api.get<{roles: string[]}>(`/api/v1/admin/users/${seed.memberId}`);
		await api.put(`/api/v1/admin/users/${seed.memberId}/roles`, {roles: [...before.roles, NAME]});
		const member = await browser.newContext({baseURL, storageState: AUTH_FILES.member});
		try {
			const memberPage = await member.newPage();
			await memberPage.goto('/settings/profile');
			await expect(memberPage.getByRole('link', {name: t('settings.tabs.roles'), exact: true})).toBeVisible();
			await memberPage.goto('/settings/roles');
			await expect(memberPage.locator(`[data-testid="role-row"][data-role-name="${NAME}"]`)).toBeVisible();
			await expect(memberPage.getByTestId('role-create')).toHaveCount(0);
			expect((await memberPage.request.post('/api/v1/admin/roles', {data: {name: 'Escalated', permissions: ['admin.*']}})).status()).toBe(403);

			await page.reload();
			await row.getByTestId('role-edit').click();
			await page.getByTestId('role-name').fill(RENAMED);
			await page.getByTestId('role-permission-admin.roles.view').click();
			await page.getByTestId('role-save').click();
			const renamed = page.locator(`[data-testid="role-row"][data-role-name="${RENAMED}"]`);
			await expect(renamed).toContainText(t('roles.user_count', {count: 1}));

			const memberView = await (await memberPage.request.get('/api/v1/users/@me')).json();
			expect(memberView.roles).toContain(RENAMED);
			expect(memberView.permissions).toContain('admin.users.view');
			expect(memberView.permissions).not.toContain('admin.roles.view');

			await renamed.getByTestId('role-delete').click();
			await page.getByTestId('role-delete-confirm').click();
			await expect(renamed).toHaveCount(0);
			const afterDelete = await (await memberPage.request.get('/api/v1/users/@me')).json();
			expect(afterDelete.roles).not.toContain(RENAMED);
			expect(afterDelete.permissions).not.toContain('admin.users.view');
		} finally {
			await member.close();
			await api.put(`/api/v1/admin/users/${seed.memberId}/roles`, {roles: before.roles});
		}
	});

	test('wildcard permissions imply the permissions they cover', async ({page}) => {
		await page.goto('/settings/roles');
		await page.getByTestId('role-create').click();
		await page.getByTestId('role-permission-admin.developer.*').click();
		await expect(page.getByTestId('role-permission-admin.developer.view')).toBeDisabled();
		await expect(page.getByTestId('role-permission-admin.developer.view')).toHaveAttribute('data-state', 'checked');
		await expect(page.getByTestId('role-permission-admin.users.view')).toBeEnabled();
	});
});
