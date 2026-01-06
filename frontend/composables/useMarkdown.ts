/**
 * Markdown rendering composable.
 *
 * Uses:
 * - `remend` for self-healing markdown during streaming
 * - `marked` for parsing markdown to HTML
 * - `shiki` for syntax highlighting in code blocks
 */
import {Marked} from 'marked';
import remend from 'remend';
import {createHighlighter, type Highlighter, type BundledLanguage} from 'shiki';

// Singleton highlighter instance
let highlighter: Highlighter | null = null;
let highlighterPromise: Promise<Highlighter> | null = null;

// Languages to preload for common use cases
const PRELOAD_LANGUAGES: BundledLanguage[] = ['javascript', 'typescript', 'python', 'rust', 'html', 'css', 'json', 'markdown', 'bash', 'sql', 'vue', 'jsx', 'tsx'];

/**
 * Initialize the shiki highlighter (lazy, singleton).
 */
async function getHighlighter(): Promise<Highlighter> {
	if (highlighter) return highlighter;

	if (!highlighterPromise) {
		highlighterPromise = createHighlighter({
			themes: ['github-dark', 'github-light'],
			langs: PRELOAD_LANGUAGES,
		});
	}

	highlighter = await highlighterPromise;
	return highlighter;
}

/**
 * Check if a language is supported by shiki.
 */
function isLanguageSupported(lang: string): lang is BundledLanguage {
	// Common aliases
	const aliases: Record<string, BundledLanguage> = {
		js: 'javascript',
		ts: 'typescript',
		py: 'python',
		rs: 'rust',
		sh: 'bash',
		shell: 'bash',
		yml: 'yaml',
		vue: 'vue',
	};

	const normalized = aliases[lang] || lang;
	return PRELOAD_LANGUAGES.includes(normalized as BundledLanguage);
}

/**
 * Normalize language name.
 */
function normalizeLanguage(lang: string): BundledLanguage {
	const aliases: Record<string, BundledLanguage> = {
		js: 'javascript',
		ts: 'typescript',
		py: 'python',
		rs: 'rust',
		sh: 'bash',
		shell: 'bash',
		yml: 'yaml',
	};

	return (aliases[lang] || lang || 'text') as BundledLanguage;
}

/**
 * Languages that support live preview.
 */
const PREVIEWABLE_LANGUAGES = ['html', 'css', 'javascript', 'js', 'svg'];

export function useMarkdown() {
	// Check if dark mode is active via document class
	const isDark = () => {
		if (typeof document === 'undefined') return true; // SSR default to dark
		return document.documentElement.classList.contains('dark');
	};

	// Create marked instance with custom renderer
	const createMarkedInstance = (hl: Highlighter | null) => {
		const marked = new Marked();

		marked.use({
			renderer: {
				code({text, lang}) {
					const language = normalizeLanguage(lang || 'text');
					const isPreviewable = PREVIEWABLE_LANGUAGES.includes(lang || '');

					let highlightedCode: string;

					if (hl && isLanguageSupported(language)) {
						try {
							highlightedCode = hl.codeToHtml(text, {
								lang: language,
								theme: isDark() ? 'github-dark' : 'github-light',
							});
						} catch {
							// Fallback to escaped text
							highlightedCode = `<pre class="shiki"><code>${escapeHtml(text)}</code></pre>`;
						}
					} else {
						highlightedCode = `<pre class="shiki"><code>${escapeHtml(text)}</code></pre>`;
					}

					// Wrap with our code block container using Tailwind classes
					const previewAttr = isPreviewable ? `data-previewable="true"` : '';
					const langAttr = lang ? `data-language="${escapeHtml(lang)}"` : '';

					return `<div class="relative my-4 rounded-lg overflow-hidden bg-card border border-border" ${langAttr} ${previewAttr}>
						<div class="flex items-center justify-between px-4 py-2 bg-muted border-b border-border">
							<span class="text-xs font-mono font-medium text-muted-foreground uppercase tracking-wide">${escapeHtml(lang || 'text')}</span>
							<div class="flex gap-1">
								${isPreviewable ? `<button class="code-block-preview-btn inline-flex items-center justify-center p-1.5 rounded-md text-muted-foreground hover:text-primary hover:bg-accent transition-colors" title="Preview">${ICON_PLAY}</button>` : ''}
								<button class="code-block-copy-btn inline-flex items-center justify-center p-1.5 rounded-md text-muted-foreground hover:text-foreground hover:bg-accent transition-colors" title="Copy">${ICON_COPY}</button>
							</div>
						</div>
						<div class="overflow-x-auto [&>pre]:!m-0 [&>pre]:!p-4 [&>pre]:!bg-transparent [&_code]:font-mono [&_code]:text-sm [&_code]:leading-relaxed">${highlightedCode}</div>
					</div>`;
				},
			},
		});

		return marked;
	};

	// Reactive highlighter loading
	const isHighlighterReady = ref(false);
	let markedInstance: Marked | null = null;

	// Initialize highlighter on mount
	onMounted(async () => {
		console.log('Initializing syntax highlighter...');
		try {
			const hl = await getHighlighter();
			markedInstance = createMarkedInstance(hl);
			isHighlighterReady.value = true;
		} catch (e) {
			console.error('Failed to initialize syntax highlighter:', e);
			markedInstance = createMarkedInstance(null);
			isHighlighterReady.value = true;
		}
	});

	/**
	 * Render markdown for streaming content.
	 * Uses remend to heal incomplete markdown before parsing.
	 */
	function renderStreaming(content: string): string {
		const ready = isHighlighterReady.value;

		if (!content) return '';

		const healed = remend(content);
		if (ready && markedInstance) {
			return markedInstance.parse(healed, {async: false}) as string;
		}

		return renderFallback(healed);
	}

	/**
	 * Render markdown for completed content.
	 * No healing needed since content is complete.
	 */
	function renderComplete(content: string): string {
		const ready = isHighlighterReady.value;
		if (!content) return '';
		if (ready && markedInstance) {
			return markedInstance.parse(content, {async: false}) as string;
		}

		return renderFallback(content);
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

	return {
		renderStreaming,
		renderComplete,
		isHighlighterReady,
	};
}

// Icons for code blocks
export const ICON_COPY = `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="lucide lucide-clipboard"><rect width="8" height="4" x="8" y="2" rx="1" ry="1"/><path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"/></svg>`;
export const ICON_CHECK = `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="lucide lucide-check"><path d="M20 6 9 17l-5-5"/></svg>`;
export const ICON_PLAY = `<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="lucide lucide-play"><polygon points="6 3 20 12 6 21 6 3"/></svg>`;

/**
 * Escape HTML entities.
 */
function escapeHtml(text: string): string {
	return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;').replace(/'/g, '&#039;');
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

	// Get text content (strips HTML tags from highlighted code)
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
