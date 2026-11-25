//! Pure-function document chunker for managed knowledge bases.
//!
//! Splits extracted Markdown-like text hierarchically (heading → paragraph → sentence → hard cut)
//! with overlap, page tracking, top-level section boundaries, and repeated Markdown table headers.

use crate::utils::extract::parse_page_marker;
use sha2::{Digest, Sha256};

/// Default target size of one chunk in estimated tokens.
pub const DEFAULT_CHUNK_TOKENS: usize = 400;

/// Default overlap between adjacent chunks in the same section, as a percentage of target tokens.
pub const DEFAULT_OVERLAP_PERCENT: usize = 15;

/// Parameters controlling how a document is split into chunks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkConfig {
	pub target_tokens: usize,
	pub overlap_percent: usize,
}

impl Default for ChunkConfig {
	fn default() -> Self {
		Self {
			target_tokens: DEFAULT_CHUNK_TOKENS,
			overlap_percent: DEFAULT_OVERLAP_PERCENT,
		}
	}
}

impl ChunkConfig {
	fn normalized(self) -> Self {
		Self {
			target_tokens: self.target_tokens.clamp(50, 4000),
			overlap_percent: self.overlap_percent.min(50),
		}
	}

	fn overlap_tokens(self) -> usize {
		(self.target_tokens * self.overlap_percent) / 100
	}
}

/// One chunk produced from a document, ready to store in `rag_chunks` and embed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextChunk {
	pub ordinal: i32,
	pub heading_path: Option<String>,
	pub page: Option<i32>,
	pub content: String,
	pub content_hash: String,
	pub token_count: i32,
}

impl TextChunk {
	/// Text sent to the embedding model: prefixed with the document title and heading path.
	#[must_use]
	pub fn embed_text(&self, document_title: &str) -> String {
		match &self.heading_path {
			Some(path) if !path.is_empty() => format!("{document_title} › {path}\n\n{}", self.content),
			_ => format!("{document_title}\n\n{}", self.content),
		}
	}
}

/// Rough token estimate (~4 UTF-8 bytes per token), matching budget estimation.
#[must_use]
pub fn estimate_tokens(text: &str) -> usize {
	if text.is_empty() { 0 } else { text.len().div_ceil(4) }
}

enum Block {
	Heading { level: usize, title: String, page: Option<i32> },
	Table { header: String, rows: Vec<String>, page: Option<i32> },
	Paragraph { text: String, page: Option<i32> },
}

/// Splits `text` into chunks using `config`.
#[must_use]
pub fn chunk_document(text: &str, config: ChunkConfig) -> Vec<TextChunk> {
	let config = config.normalized();
	let blocks = parse_blocks(text);
	let mut state = ChunkBuilder::new(config);
	for block in blocks {
		state.push_block(block);
	}
	state.finish()
}

struct ChunkBuilder {
	config: ChunkConfig,
	headings: Vec<(usize, String)>,
	buffer: String,
	buffer_page: Option<i32>,
	buffer_heading: Option<String>,
	buffer_has_body: bool,
	carry_overlap: Option<String>,
	chunks: Vec<TextChunk>,
}

impl ChunkBuilder {
	fn new(config: ChunkConfig) -> Self {
		Self {
			config,
			headings: Vec::new(),
			buffer: String::new(),
			buffer_page: None,
			buffer_heading: None,
			buffer_has_body: false,
			carry_overlap: None,
			chunks: Vec::new(),
		}
	}

	fn heading_path(&self) -> Option<String> {
		if self.headings.is_empty() {
			None
		} else {
			Some(self.headings.iter().map(|(_, title)| title.as_str()).collect::<Vec<_>>().join(" › "))
		}
	}

	fn push_block(&mut self, block: Block) {
		match block {
			Block::Heading { level, title, page } => self.push_heading(level, title, page),
			Block::Table { header, rows, page } => self.push_table(&header, &rows, page),
			Block::Paragraph { text, page } => self.push_paragraph(&text, page),
		}
	}

	fn push_heading(&mut self, level: usize, title: String, page: Option<i32>) {
		if level <= 2 && self.buffer_has_body {
			self.flush(false);
		}
		if level <= 2 {
			self.carry_overlap = None;
		}
		while self.headings.last().is_some_and(|(existing, _)| *existing >= level) {
			self.headings.pop();
		}
		self.headings.push((level, title.clone()));
		let line = format!("{} {title}", "#".repeat(level));
		self.push_unit(&line, page, "\n\n", false);
		if !self.buffer_has_body {
			self.buffer_heading = self.heading_path();
		}
	}

	fn push_table(&mut self, header: &str, rows: &[String], page: Option<i32>) {
		if !self.buffer.is_empty() {
			let first_row_tokens = rows.first().map_or(0, |row| estimate_tokens(row));
			let combined = estimate_tokens(&self.buffer) + estimate_tokens(header) + first_row_tokens + 2;
			if combined > self.config.target_tokens {
				self.flush(false);
			}
		}
		self.carry_overlap = None;
		let full = if rows.is_empty() { header.to_owned() } else { format!("{header}\n{}", rows.join("\n")) };
		if estimate_tokens(&full) <= self.config.target_tokens {
			self.push_unit(&full, page, "\n\n", true);
			return;
		}
		if self.buffer_has_body {
			self.flush(false);
		} else {
			self.buffer.clear();
			self.buffer_page = None;
			self.buffer_heading = None;
		}
		let mut part = header.to_owned();
		let mut has_rows = false;
		for row in rows {
			let candidate_tokens = estimate_tokens(&part) + estimate_tokens(row) + 1;
			if has_rows && candidate_tokens > self.config.target_tokens {
				self.emit_raw(&part, page);
				part = header.to_owned();
			}
			part.push('\n');
			part.push_str(row);
			has_rows = true;
		}
		if has_rows {
			self.emit_raw(&part, page);
		}
	}

	fn push_paragraph(&mut self, text: &str, page: Option<i32>) {
		if estimate_tokens(text) <= self.config.target_tokens {
			self.push_unit(text, page, "\n\n", true);
			return;
		}
		for sentence in split_sentences(text) {
			if estimate_tokens(&sentence) <= self.config.target_tokens {
				self.push_unit(&sentence, page, " ", true);
			} else {
				for piece in hard_split(&sentence, self.config.target_tokens) {
					self.push_unit(&piece, page, " ", true);
				}
			}
		}
	}

	fn push_unit(&mut self, unit: &str, page: Option<i32>, separator: &str, is_body: bool) {
		let unit = unit.trim();
		if unit.is_empty() {
			return;
		}
		if self.buffer.is_empty() {
			self.start_buffer(unit, page, is_body);
			return;
		}
		let projected = estimate_tokens(&self.buffer) + estimate_tokens(unit) + 1;
		if projected > self.config.target_tokens {
			self.flush(true);
			self.start_buffer(unit, page, is_body);
		} else {
			self.buffer.push_str(separator);
			self.buffer.push_str(unit);
			if is_body {
				self.buffer_has_body = true;
			}
		}
	}

	fn start_buffer(&mut self, unit: &str, page: Option<i32>, is_body: bool) {
		self.buffer_page = page;
		self.buffer_heading = self.heading_path();
		self.buffer_has_body = is_body;
		if let Some(overlap) = self.carry_overlap.take()
			&& estimate_tokens(&overlap) + estimate_tokens(unit) < self.config.target_tokens
		{
			self.buffer.push_str(&overlap);
			self.buffer.push_str("\n\n");
		}
		self.buffer.push_str(unit);
	}

	fn flush(&mut self, keep_overlap: bool) {
		let content = std::mem::take(&mut self.buffer);
		let page = self.buffer_page.take();
		let heading = self.buffer_heading.take();
		self.buffer_has_body = false;
		let trimmed = content.trim();
		if trimmed.is_empty() {
			return;
		}
		if keep_overlap {
			self.carry_overlap = tail_overlap(trimmed, self.config.overlap_tokens());
		} else {
			self.carry_overlap = None;
		}
		self.push_chunk(trimmed.to_owned(), heading, page);
	}

	fn emit_raw(&mut self, content: &str, page: Option<i32>) {
		let trimmed = content.trim();
		if trimmed.is_empty() {
			return;
		}
		let heading = self.heading_path();
		self.push_chunk(trimmed.to_owned(), heading, page);
	}

	fn push_chunk(&mut self, content: String, heading_path: Option<String>, page: Option<i32>) {
		let ordinal = i32::try_from(self.chunks.len()).unwrap_or(i32::MAX);
		let token_count = i32::try_from(estimate_tokens(&content)).unwrap_or(i32::MAX);
		let content_hash = sha256_hex(content.as_bytes());
		self.chunks.push(TextChunk {
			ordinal,
			heading_path,
			page,
			content,
			content_hash,
			token_count,
		});
	}

	fn finish(mut self) -> Vec<TextChunk> {
		self.flush(false);
		self.chunks
	}
}

/// SHA-256 lowercase hex digest.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
	let digest = Sha256::digest(bytes);
	let mut hex = String::with_capacity(64);
	for byte in digest {
		use std::fmt::Write as _;
		let _ = write!(hex, "{byte:02x}");
	}
	hex
}

fn parse_blocks(text: &str) -> Vec<Block> {
	let lines: Vec<&str> = text.lines().collect();
	let mut blocks = Vec::new();
	let mut index = 0;
	let mut current_page = None;
	let mut paragraph = Vec::new();
	let mut para_page = None;
	let mut in_fence = false;

	while index < lines.len() {
		let line = lines[index];
		let trimmed = line.trim();
		if let Some(page) = parse_page_marker(trimmed) {
			flush_paragraph(&mut blocks, &mut paragraph, &mut para_page);
			current_page = Some(page);
			index += 1;
			continue;
		}
		if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
			in_fence = !in_fence;
			if paragraph.is_empty() {
				para_page = current_page;
			}
			paragraph.push(line);
			index += 1;
			continue;
		}
		if !in_fence {
			if let Some((level, title)) = parse_heading(trimmed) {
				flush_paragraph(&mut blocks, &mut paragraph, &mut para_page);
				blocks.push(Block::Heading {
					level,
					title: title.to_owned(),
					page: current_page,
				});
				index += 1;
				continue;
			}
			if is_table_start(&lines, index) {
				flush_paragraph(&mut blocks, &mut paragraph, &mut para_page);
				let (table, consumed) = consume_table(&lines, index, current_page);
				blocks.push(table);
				index += consumed;
				continue;
			}
			if trimmed.is_empty() {
				flush_paragraph(&mut blocks, &mut paragraph, &mut para_page);
				index += 1;
				continue;
			}
		}
		if paragraph.is_empty() {
			para_page = current_page;
		}
		paragraph.push(line);
		index += 1;
	}
	flush_paragraph(&mut blocks, &mut paragraph, &mut para_page);
	blocks
}

fn flush_paragraph(blocks: &mut Vec<Block>, lines: &mut Vec<&str>, page: &mut Option<i32>) {
	if lines.is_empty() {
		return;
	}
	let text = lines.join("\n");
	lines.clear();
	let para_page = page.take();
	if !text.trim().is_empty() {
		blocks.push(Block::Paragraph { text, page: para_page });
	}
}

fn parse_heading(line: &str) -> Option<(usize, &str)> {
	let hashes = line.bytes().take_while(|byte| *byte == b'#').count();
	if !(1..=6).contains(&hashes) {
		return None;
	}
	let rest = line[hashes..].strip_prefix(' ')?.trim();
	let title = rest.trim_end_matches('#').trim();
	(!title.is_empty()).then_some((hashes, title))
}

fn is_table_start(lines: &[&str], index: usize) -> bool {
	let Some(header) = lines.get(index) else {
		return false;
	};
	let Some(separator) = lines.get(index + 1) else {
		return false;
	};
	header.contains('|') && is_table_separator(separator)
}

fn is_table_separator(line: &str) -> bool {
	let trimmed = line.trim();
	trimmed.contains('-') && trimmed.contains('|') && trimmed.bytes().all(|byte| matches!(byte, b'|' | b'-' | b':' | b' ' | b'\t'))
}

fn consume_table(lines: &[&str], start: usize, page: Option<i32>) -> (Block, usize) {
	let header = format!("{}\n{}", lines[start].trim(), lines[start + 1].trim());
	let mut rows = Vec::new();
	let mut cursor = start + 2;
	while let Some(line) = lines.get(cursor) {
		let trimmed = line.trim();
		if trimmed.is_empty() || !trimmed.contains('|') || parse_page_marker(trimmed).is_some() {
			break;
		}
		rows.push(trimmed.to_owned());
		cursor += 1;
	}
	(Block::Table { header, rows, page }, cursor - start)
}

fn split_sentences(text: &str) -> Vec<String> {
	let mut result = Vec::new();
	let mut start = 0;
	let bytes = text.as_bytes();
	let mut index = 0;
	while index < bytes.len() {
		let end_sentence = matches!(bytes[index], b'.' | b'!' | b'?') && bytes.get(index + 1).is_some_and(u8::is_ascii_whitespace);
		let newline = bytes[index] == b'\n';
		if end_sentence || newline {
			let end = index + 1;
			let slice = text[start..end].trim();
			if !slice.is_empty() {
				result.push(slice.to_owned());
			}
			start = end;
		}
		index += 1;
	}
	if start < text.len() {
		let slice = text[start..].trim();
		if !slice.is_empty() {
			result.push(slice.to_owned());
		}
	}
	result
}

fn hard_split(text: &str, target_tokens: usize) -> Vec<String> {
	let max_bytes = target_tokens.saturating_mul(4).max(32);
	let mut parts = Vec::new();
	let mut rest = text.trim();
	while !rest.is_empty() {
		if rest.len() <= max_bytes {
			parts.push(rest.to_owned());
			break;
		}
		let mut split_at = max_bytes;
		while split_at > 0 && !rest.is_char_boundary(split_at) {
			split_at -= 1;
		}
		let cut = rest[..split_at].rfind(char::is_whitespace).filter(|pos| *pos > split_at / 2).unwrap_or(split_at);
		let cut = if cut == 0 {
			rest.char_indices().nth(1).map_or(rest.len(), |(idx, _)| idx)
		} else {
			cut
		};
		let head = rest[..cut].trim();
		if !head.is_empty() {
			parts.push(head.to_owned());
		}
		rest = rest[cut..].trim();
	}
	parts
}

fn tail_overlap(text: &str, overlap_tokens: usize) -> Option<String> {
	if overlap_tokens == 0 {
		return None;
	}
	let max_bytes = overlap_tokens.saturating_mul(4);
	if text.len() <= max_bytes {
		return Some(text.to_owned());
	}
	let mut start = text.len().saturating_sub(max_bytes);
	while start < text.len() && !text.is_char_boundary(start) {
		start += 1;
	}
	let adjusted = text[start..].find(char::is_whitespace).map_or(start, |offset| start + offset + 1);
	let slice = text.get(adjusted..).unwrap_or("").trim();
	(!slice.is_empty()).then(|| slice.to_owned())
}
