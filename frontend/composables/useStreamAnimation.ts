import type {StreamingAnimation} from '~/types/chat';

export const STREAMING_ANIMATIONS: StreamingAnimation[] = ['fade', 'slide', 'typewriter', 'none'];

const CHUNK_MS = 400;
const TYPE_FRAMES = 12;

type Key = string | number;
type Mark = {end: number; at: number};

function reducedMotion(): boolean {
	return typeof window !== 'undefined' && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
}

function textNodes(root: Node): Text[] {
	const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
	const nodes: Text[] = [];
	while (walker.nextNode()) nodes.push(walker.currentNode as Text);
	return nodes;
}

function chunkSpan(text: string, age: number, animation: string): HTMLSpanElement {
	const span = document.createElement('span');
	span.className = `stream-${animation}`;
	span.style.animationDelay = `-${Math.round(age)}ms`;
	span.textContent = text;
	return span;
}

/**
 * Wraps the text that arrived in the last few hundred milliseconds in spans
 * that fade or slide in. Each span carries a negative delay equal to its age,
 * so re-rendering the message continues an animation instead of restarting it.
 */
function wrapRecent(template: HTMLTemplateElement, marks: Mark[], now: number, animation: string) {
	const settled = marks[0]!.end;
	let offset = 0;
	for (const node of textNodes(template.content)) {
		const text = node.data;
		const start = offset;
		offset += text.length;
		if (offset <= settled || !text.trim()) continue;
		const pieces = document.createDocumentFragment();
		let cut = Math.max(0, settled - start);
		if (cut > 0) pieces.append(text.slice(0, cut));
		for (const mark of marks.slice(1)) {
			const end = Math.min(text.length, mark.end - start);
			if (end <= cut) continue;
			pieces.append(chunkSpan(text.slice(cut, end), now - mark.at, animation));
			cut = end;
		}
		if (cut < text.length) pieces.append(text.slice(cut));
		node.replaceWith(pieces);
	}
}

function appendCaret(template: HTMLTemplateElement) {
	const caret = document.createElement('span');
	caret.className = 'stream-caret';
	const last = textNodes(template.content).at(-1);
	if (last) last.after(caret);
	else template.content.append(caret);
}

/**
 * Streaming animation for chat replies, picked by the user's
 * `streaming_animation` preference.
 *
 * `reveal` holds text back for the typewriter, which types it out at a pace
 * that keeps up with the stream. `decorate` adds the fade or slide spans, or
 * the typewriter caret, to the rendered HTML. Both take a key per text part.
 */
export function useStreamAnimation(mode: () => string | undefined) {
	const shown = reactive(new Map<Key, number>());
	const targets = new Map<Key, number>();
	const marks = new Map<Key, Mark[]>();
	let frame = 0;

	const active = () => (reducedMotion() ? 'none' : mode() ?? 'fade');

	function type() {
		frame = 0;
		for (const [key, target] of targets) {
			const count = shown.get(key) ?? 0;
			if (count >= target) continue;
			shown.set(key, count + Math.ceil((target - count) / TYPE_FRAMES));
			frame ||= requestAnimationFrame(type);
		}
	}

	function reveal(key: Key, text: string): string {
		if (active() !== 'typewriter') return text;
		targets.set(key, text.length);
		let count = Math.min(shown.get(key) ?? 0, text.length);
		if (count < text.length) {
			frame ||= requestAnimationFrame(type);
			if (/[\uD800-\uDBFF]/.test(text.charAt(count - 1))) count += 1;
		}
		return text.slice(0, count);
	}

	function recentMarks(key: Key, length: number, now: number): Mark[] {
		const list = marks.get(key) ?? [{end: 0, at: 0}];
		if (length > list.at(-1)!.end) list.push({end: length, at: now});
		while (list.length > 1 && now - list[1]!.at >= CHUNK_MS) list.shift();
		marks.set(key, list);
		return list;
	}

	function decorate(key: Key, html: string): string {
		const animation = active();
		if (!html || animation === 'none' || typeof document === 'undefined') return html;
		const template = document.createElement('template');
		template.innerHTML = html;
		if (animation === 'typewriter') {
			appendCaret(template);
		} else {
			const now = performance.now();
			const list = recentMarks(key, template.content.textContent?.length ?? 0, now);
			if (list.length < 2) return html;
			wrapRecent(template, list, now, animation);
		}
		return template.innerHTML;
	}

	onBeforeUnmount(() => cancelAnimationFrame(frame));

	return {reveal, decorate, animation: computed(active)};
}
