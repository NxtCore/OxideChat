/**
 * Markdown rendering composable.
 *
 * Uses:
 * - `remend` for self-healing markdown during streaming
 * - `marked` for parsing markdown to HTML
 * - `nuxt-shiki` for syntax highlighting in code blocks
 * - `DOMPurify` for sanitizing HTML output to prevent XSS
 */
import DOMPurify from 'dompurify';
import {Marked} from 'marked';
import remend from 'remend';

function sanitize(html: string): string {
	return DOMPurify.sanitize(html, {
		ADD_ATTR: ['data-language', 'data-previewable', 'title', 'src', 'alt'],
		ADD_TAGS: ['button', 'img'],
		ALLOWED_URI_REGEXP: /^(?:(?:(?:f|ht)tps?|mailto|tel|callto|sms|cid|xmpp|data):|[^a-z]|[a-z+.\-]+(?:[^a-z+.\-:]|$))/i,
	});
}

// Language aliases for normalization
const LANG_ALIASES: Record<string, string> = {
	js: 'javascript',
	ts: 'typescript',
	py: 'python',
	rs: 'rust',
	sh: 'bash',
	shell: 'bash',
	yml: 'yaml',
};

/**
 * Normalize language name.
 */
function normalizeLanguage(lang: string): string {
	return LANG_ALIASES[lang] || lang || 'text';
}

/**
 * Languages that support live preview.
 */
const PREVIEWABLE_LANGUAGES = ['html', 'svg'];

export const ICON_COPY = `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="lucide lucide-clipboard"><rect width="8" height="4" x="8" y="2" rx="1" ry="1"/><path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"/></svg>`;
export const ICON_CHECK = `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="lucide lucide-check"><path d="M20 6 9 17l-5-5"/></svg>`;
export const ICON_PLAY = `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="lucide lucide-play"><polygon points="6 3 20 12 6 21 6 3"/></svg>`;

/**
 * Escape HTML entities.
 */
function escapeHtml(text: string): string {
	return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;').replace(/'/g, '&#039;');
}

const isDark = () => {
	if (typeof document === 'undefined') return true;
	return document.body.classList.contains('dark');
};

// Global highlighter state
let highlighterInstance: Awaited<ReturnType<typeof getShikiHighlighter>> | null = null;
let highlighterPromise: Promise<Awaited<ReturnType<typeof getShikiHighlighter>>> | null = null;
const globalHighlighterReady = ref(false);
let globalMarkedInstance: Marked | null = null;

/**
 * Get the shiki highlighter instance (lazy singleton).
 */
async function ensureHighlighter() {
	if (highlighterInstance) return highlighterInstance;

	if (!highlighterPromise) {
		highlighterPromise = getShikiHighlighter();
	}

	highlighterInstance = await highlighterPromise;
	return highlighterInstance;
}

/**
 * Create a marked instance with shiki syntax highlighting.
 */
function createMarkedInstance(hl: Awaited<ReturnType<typeof getShikiHighlighter>>) {
	const marked = new Marked();

	marked.use({
		renderer: {
			code({text, lang}) {
				const language = normalizeLanguage(lang || 'text');
				const isPreviewable = PREVIEWABLE_LANGUAGES.includes(lang || '');

				let highlightedCode: string;

				try {
					highlightedCode = hl.highlight(text, {
						lang: language,
						theme: isDark() ? 'github-dark' : 'github-light',
					});
				} catch {
					// Fallback for unsupported languages
					highlightedCode = `<pre class="shiki"><code>${escapeHtml(text)}</code></pre>`;
				}

				const previewAttr = isPreviewable ? `data-previewable="true"` : '';
				const langAttr = lang ? `data-language="${escapeHtml(lang)}"` : '';

				return `<div class="code-block-wrapper relative my-4 rounded-lg overflow-hidden bg-card border border-border" ${langAttr} ${previewAttr}>
					<div class="flex items-center justify-between px-4 py-2 bg-muted border-b border-border">
						<span class="text-xs font-mono font-medium text-muted-foreground uppercase tracking-wide">${escapeHtml(lang || 'text')}</span>
						<div class="flex gap-1">
							${isPreviewable ? `<button class="code-block-preview-btn inline-flex items-center justify-center p-1.5 rounded-md text-muted-foreground hover:text-primary hover:bg-accent transition-colors" title="Preview">${ICON_PLAY}</button>` : ''}
							<button class="code-block-copy-btn inline-flex items-center justify-center p-1.5 rounded-md text-muted-foreground hover:text-foreground hover:bg-accent transition-colors" title="Copy">${ICON_COPY}</button>
						</div>
					</div>
					<div class="overflow-x-auto [&>pre]:m-0! [&>pre]:p-4! [&>pre]:bg-transparent! [&_code]:font-mono [&_code]:text-sm [&_code]:leading-relaxed">${highlightedCode}</div>
				</div>`;
			},
		},
	});

	return marked;
}

/**
 * Initialize the syntax highlighter globally (called once).
 */
let initializationStarted = false;
async function initializeHighlighter(): Promise<void> {
	if (initializationStarted) return;
	initializationStarted = true;

	console.log('Initializing nuxt-shiki highlighter...');
	try {
		const hl = await ensureHighlighter();
		globalMarkedInstance = createMarkedInstance(hl);
		globalHighlighterReady.value = true;
		console.log('Syntax highlighter ready');
	} catch (e) {
		console.error('Failed to initialize syntax highlighter:', e);
		// Create a fallback marked instance without highlighting
		globalMarkedInstance = new Marked();
		globalHighlighterReady.value = true;
	}
}

/**
 * Fallback renderer when marked isn't ready.
 */
function renderFallback(text: string): string {
	return text
		.replace(/&/g, '&amp;')
		.replace(/</g, '&lt;')
		.replace(/>/g, '&gt;')
		.replace(
			/```(\w*)\n([\s\S]*?)```/g,
			'<pre class="my-4 p-4 bg-muted rounded-lg overflow-x-auto border border-border"><code class="font-mono text-sm leading-relaxed">$2</code></pre>'
		)
		.replace(/`([^`]+)`/g, '<code class="px-1.5 py-0.5 bg-muted rounded text-sm font-mono">$1</code>')
		.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')
		.replace(/\*([^*]+)\*/g, '<em>$1</em>')
		.replace(/\n/g, '<br>');
}

export function useMarkdown() {
	// Trigger initialization if not already started
	if (!globalHighlighterReady.value && !initializationStarted) {
		initializeHighlighter();
	}

	/**
	 * Render markdown for streaming content.
	 * Uses remend to heal incomplete markdown before parsing.
	 */
	function renderStreaming(content: string): string {
		// Access the reactive ref to ensure Vue tracks this dependency
		const ready = globalHighlighterReady.value;

		if (!content) return '';

		const healed = remend(content);
		if (ready && globalMarkedInstance) {
			return sanitize(globalMarkedInstance.parse(healed, {async: false}) as string);
		}

		return sanitize(renderFallback(healed));
	}

	/**
	 * Render markdown for completed content.
	 * No healing needed since content is complete.
	 */
	function renderComplete(content: string): string {
		// Access the reactive ref to ensure Vue tracks this dependency
		const ready = globalHighlighterReady.value;
		if (!content) return '';
		if (ready && globalMarkedInstance) {
			return sanitize(globalMarkedInstance.parse(content, {async: false}) as string);
		}

		return sanitize(renderFallback(content));
	}

	return {
		renderStreaming,
		renderComplete,
		isHighlighterReady: globalHighlighterReady,
	};
}

/**
 * Extract code from a code block element for preview.
 */
export function extractCodeForPreview(element: HTMLElement): {code: string; language: string} | null {
	const wrapper = element.closest('.code-block-wrapper');
	if (!wrapper) return null;

	const language = wrapper.getAttribute('data-language') || 'text';
	const codeEl = wrapper.querySelector('code');
	if (!codeEl) return null;

	const code = codeEl.textContent || '';

	return {code, language};
}

/**
 * Generate HTML for a preview iframe.
 */
export function generatePreviewHtml(code: string, language: string): string {
	if (language === 'html' || language === 'svg') {
		return code;
	}

	if (language === 'css') {
		return `<!DOCTYPE html>
<html>
<head><style>${code}</style></head>
<body><div class="preview">CSS Preview - Add HTML to see styled content</div></body>
</html>`;
	}

	if (language === 'javascript' || language === 'js') {
		return `<!DOCTYPE html>
<html>
<head></head>
<body>
<div id="output"></div>
<script>
try {
  ${code}
} catch(e) {
  document.getElementById('output').textContent = 'Error: ' + e.message;
}
</script>
</body>
</html>`;
	}

	return `<pre>${escapeHtml(code)}</pre>`;
}
