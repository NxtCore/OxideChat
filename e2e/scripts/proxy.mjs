import http from 'node:http';

const port = Number(process.env.E2E_PORT ?? 3100);
const backend = {host: '127.0.0.1', port: Number(process.env.E2E_BACKEND_PORT ?? 3101)};
const frontend = {host: '127.0.0.1', port: Number(process.env.E2E_FRONTEND_PORT ?? 3102)};

function targetFor(path) {
	if (path.startsWith('/api/') || path.startsWith('/openai/') || path === '/health') return backend;
	return frontend;
}

const server = http.createServer((req, res) => {
	const target = targetFor(req.url ?? '/');
	const upstream = http.request(
		{...target, method: req.method, path: req.url, headers: {...req.headers, 'x-forwarded-for': req.socket.remoteAddress ?? ''}},
		upstreamRes => {
			res.writeHead(upstreamRes.statusCode ?? 502, upstreamRes.headers);
			upstreamRes.pipe(res);
		},
	);
	upstream.on('error', error => {
		if (!res.headersSent) res.writeHead(502, {'content-type': 'text/plain'});
		res.end(`proxy error: ${error.message}`);
	});
	res.on('close', () => upstream.destroy());
	req.pipe(upstream);
});

server.listen(port, '127.0.0.1', () => console.log(`[proxy] http://127.0.0.1:${port} -> api ${backend.port}, ui ${frontend.port}`));
