import {spawn} from 'node:child_process';
import {existsSync, mkdirSync, rmSync} from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import pg from 'pg';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const database = process.env.E2E_PG_DB ?? 'oxidechat_e2e';
const connection = {
	host: process.env.E2E_PG_HOST ?? '127.0.0.1',
	port: Number(process.env.E2E_PG_PORT ?? 5443),
	user: process.env.E2E_PG_USER ?? 'oxide',
	password: process.env.E2E_PG_PASSWORD ?? 'e2e',
};

if (!/^[a-z0-9_]+$/.test(database) || !database.includes('e2e')) {
	throw new Error(`Refusing to reset database "${database}": e2e database names must contain "e2e"`);
}

const admin = new pg.Client({...connection, database: 'postgres'});
await admin.connect();
await admin.query(`DROP DATABASE IF EXISTS ${database} WITH (FORCE)`);
await admin.query(`CREATE DATABASE ${database}`);
await admin.end();
console.log(`[e2e] reset database ${database} on ${connection.host}:${connection.port}`);

const dataDir = path.join(root, 'e2e', '.data');
rmSync(dataDir, {recursive: true, force: true});
mkdirSync(dataDir, {recursive: true});

const binary = process.env.E2E_BACKEND_BIN ?? path.join(root, 'target', 'debug', process.platform === 'win32' ? 'OxideChat.exe' : 'OxideChat');
if (!existsSync(binary)) {
	throw new Error(`Backend binary not found at ${binary}. Run "cargo build" first or set E2E_BACKEND_BIN.`);
}

const child = spawn(binary, [], {
	cwd: root,
	stdio: 'inherit',
	env: {
		...process.env,
		POSTGRES_HOST: connection.host,
		POSTGRES_PORT: String(connection.port),
		POSTGRES_DB: database,
		POSTGRES_USER: connection.user,
		POSTGRES_PASSWORD: connection.password,
		HOST: '127.0.0.1',
		PORT: process.env.E2E_BACKEND_PORT ?? '3101',
		BASE_URL: `http://localhost:${process.env.E2E_PORT ?? 3100}`,
		OXIDECHAT_DATA_DIR: dataDir,
		ENCRYPTION_KEY: '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
		OAUTH_GOOGLE_CLIENT_ID: '',
		OAUTH_GOOGLE_CLIENT_SECRET: '',
		OAUTH_DISCORD_CLIENT_ID: '',
		OAUTH_DISCORD_CLIENT_SECRET: '',
		RUST_LOG: process.env.RUST_LOG ?? 'warn',
	},
});

const stop = () => child.kill();
process.on('SIGINT', stop);
process.on('SIGTERM', stop);
child.on('exit', code => process.exit(code ?? 0));
