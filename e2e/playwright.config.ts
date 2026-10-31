import {defineConfig, devices} from '@playwright/test';

const port = Number(process.env.E2E_PORT ?? 3100);
const backendPort = Number(process.env.E2E_BACKEND_PORT ?? 3101);
const frontendPort = Number(process.env.E2E_FRONTEND_PORT ?? 3102);
const mockPort = Number(process.env.MOCK_LLM_PORT ?? 3103);

export default defineConfig({
	testDir: './tests',
	outputDir: './test-results',
	fullyParallel: false,
	workers: Number(process.env.E2E_WORKERS ?? 1),
	retries: process.env.CI ? 1 : 0,
	timeout: 60_000,
	expect: {timeout: 10_000},
	forbidOnly: !!process.env.CI,
	reporter: process.env.CI ? [['github'], ['html', {open: 'never'}]] : [['list'], ['html', {open: 'never'}]],
	use: {
		baseURL: `http://localhost:${port}`,
		locale: 'en-US',
		timezoneId: 'UTC',
		trace: 'retain-on-failure',
		screenshot: 'only-on-failure',
		video: 'retain-on-failure',
	},
	projects: [
		{name: 'setup', testMatch: /.*\.setup\.ts/},
		{
			name: 'chromium',
			use: {...devices['Desktop Chrome'], storageState: '.auth/admin.json'},
			dependencies: ['setup'],
		},
	],
	webServer: [
		{
			command: 'bun scripts/mock-llm.mjs',
			url: `http://127.0.0.1:${mockPort}/health`,
			reuseExistingServer: false,
			stdout: 'ignore',
		},
		{
			command: 'bun scripts/start-backend.mjs',
			url: `http://127.0.0.1:${backendPort}/api/v1/health`,
			reuseExistingServer: false,
			timeout: 120_000,
			stdout: 'pipe',
		},
		{
			command: 'bun scripts/start-frontend.mjs',
			url: `http://127.0.0.1:${frontendPort}`,
			reuseExistingServer: false,
			timeout: 300_000,
		},
		{
			command: 'bun scripts/proxy.mjs',
			url: `http://127.0.0.1:${port}/api/v1/health`,
			reuseExistingServer: false,
		},
	],
});
