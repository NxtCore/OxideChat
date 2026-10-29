import {spawn, spawnSync} from 'node:child_process';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

const frontend = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..', 'frontend');
const shell = process.platform === 'win32';

if (process.env.E2E_SKIP_BUILD !== '1') {
	const build = spawnSync('bun', ['run', 'build'], {cwd: frontend, stdio: 'inherit', shell});
	if (build.status !== 0) process.exit(build.status ?? 1);
}

const child = spawn('bun', [path.join('.output', 'server', 'index.mjs')], {
	cwd: frontend,
	stdio: 'inherit',
	shell,
	env: {...process.env, HOST: '127.0.0.1', NITRO_HOST: '127.0.0.1', PORT: process.env.E2E_FRONTEND_PORT ?? '3102', NITRO_PORT: process.env.E2E_FRONTEND_PORT ?? '3102'},
});

const stop = () => child.kill();
process.on('SIGINT', stop);
process.on('SIGTERM', stop);
child.on('exit', code => process.exit(code ?? 0));
