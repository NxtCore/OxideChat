//! Text extraction for uploaded files.
//!
//! HTML and PDF parsers run in a child process (this binary with the `extract` subcommand) with a time
//! and memory limit and no environment, so a hanging parser or a compression bomb is killed instead of
//! taking the server down.

use std::io::{Read, Write};
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::sync::Semaphore;

/// Largest file a user may upload.
pub const MAX_UPLOAD_BYTES: usize = 25 * 1024 * 1024;

/// Largest text extraction may yield.
pub const MAX_TEXT_BYTES: usize = 8 * 1024 * 1024;

/// The command-line argument that starts the extraction child process.
pub const SUBCOMMAND: &str = "extract";

const TIMEOUT: Duration = Duration::from_secs(30);

const MAX_CONCURRENT_CHILDREN: usize = 4;

static CHILD_SLOTS: Semaphore = Semaphore::const_new(MAX_CONCURRENT_CHILDREN);

#[cfg(target_os = "linux")]
const MEMORY_LIMIT_BYTES: libc::rlim_t = 1024 * 1024 * 1024;

const HTML_WIDTH: usize = 1000;

const TEXT_EXTENSIONS: &[&str] = &[
	"txt", "md", "markdown", "csv", "tsv", "json", "jsonl", "yaml", "yml", "xml", "log", "ini", "toml", "sql", "rs", "py", "js", "mjs", "ts", "tsx", "jsx", "vue", "go",
	"java", "kt", "c", "h", "cpp", "hpp", "cs", "rb", "php", "sh", "css", "scss", "swift",
];

/// Why text could not be extracted from an uploaded file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ExtractError {
	#[error("file is not valid UTF-8 text")]
	NotText,
	#[error("file yielded no readable text")]
	Empty,
	#[error("extracted text exceeds limit")]
	TextTooLong,
	#[error("extraction timed out")]
	TimedOut,
	#[error("extraction is busy")]
	Busy,
	#[error("extraction failed")]
	ExtractFailed,
}

impl ExtractError {
	/// Stable machine-readable reason suffix for document errors and translations.
	#[must_use]
	pub const fn reason(self) -> &'static str {
		match self {
			Self::NotText => "not_text",
			Self::Empty => "empty",
			Self::TextTooLong => "text_too_long",
			Self::TimedOut => "timed_out",
			Self::Busy => "busy",
			Self::ExtractFailed => "extract_failed",
		}
	}
}

/// How a file is turned into text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentKind {
	Text,
	Html,
	Pdf,
}

impl DocumentKind {
	/// The kind of a file from its extension; a PDF must also start with `%PDF-`.
	#[must_use]
	pub fn detect(filename: &str, bytes: &[u8]) -> Option<Self> {
		let extension = Path::new(filename).extension()?.to_str()?.to_ascii_lowercase();
		match extension.as_str() {
			"pdf" => bytes.starts_with(b"%PDF-").then_some(Self::Pdf),
			"html" | "htm" => Some(Self::Html),
			other if TEXT_EXTENSIONS.contains(&other) => Some(Self::Text),
			_ => None,
		}
	}

	/// The media type the document is stored with.
	#[must_use]
	pub const fn mime_type(self) -> &'static str {
		match self {
			Self::Text => "text/plain",
			Self::Html => "text/html",
			Self::Pdf => "application/pdf",
		}
	}

	/// Reconstructs the kind from a stored media type.
	#[must_use]
	pub fn from_mime(mime: &str) -> Option<Self> {
		match mime {
			"text/plain" => Some(Self::Text),
			"text/html" => Some(Self::Html),
			"application/pdf" => Some(Self::Pdf),
			_ => None,
		}
	}

	const fn arg(self) -> &'static str {
		match self {
			Self::Text => "text",
			Self::Html => "html",
			Self::Pdf => "pdf",
		}
	}

	fn from_arg(arg: &str) -> Option<Self> {
		[Self::Text, Self::Html, Self::Pdf].into_iter().find(|kind| kind.arg() == arg)
	}
}

/// Extracts the text of an uploaded file; PDF pages are marked with `<!-- page N -->`.
///
/// # Errors
///
/// Returns an [`ExtractError`] when the file is not text, cannot be parsed in time, or yields no or too much text.
pub async fn extract(kind: DocumentKind, bytes: &[u8]) -> Result<String, ExtractError> {
	let text = match kind {
		DocumentKind::Text => decode_text(bytes)?,
		DocumentKind::Html | DocumentKind::Pdf => run_child(kind, bytes).await?,
	};
	let text = text.trim();
	if text.is_empty() {
		return Err(ExtractError::Empty);
	}
	if text.len() > MAX_TEXT_BYTES {
		return Err(ExtractError::TextTooLong);
	}
	Ok(text.to_owned())
}

/// Counts PDF page markers (`<!-- page N -->`) in extracted text, returning the highest page number if any exist.
#[must_use]
pub fn page_count(text: &str) -> Option<i32> {
	text.lines().filter_map(parse_page_marker).max()
}

/// Parses a `<!-- page N -->` marker line into its 1-based page number.
#[must_use]
pub fn parse_page_marker(line: &str) -> Option<i32> {
	let inner = line.trim().strip_prefix("<!-- page ")?.strip_suffix(" -->")?;
	inner.trim().parse::<i32>().ok().filter(|page| *page > 0)
}

fn decode_text(bytes: &[u8]) -> Result<String, ExtractError> {
	let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
	if bytes.contains(&0) {
		return Err(ExtractError::NotText);
	}
	let text = String::from_utf8(bytes.to_vec()).map_err(|_| ExtractError::NotText)?;
	Ok(text.replace("\r\n", "\n").replace('\r', "\n"))
}

async fn run_child(kind: DocumentKind, bytes: &[u8]) -> Result<String, ExtractError> {
	let _slot = tokio::time::timeout(TIMEOUT, CHILD_SLOTS.acquire())
		.await
		.map_err(|_| ExtractError::Busy)?
		.map_err(|_| ExtractError::ExtractFailed)?;
	let exe = std::env::current_exe().map_err(|error| {
		tracing::error!(%error, "could not find the server binary for text extraction");
		ExtractError::ExtractFailed
	})?;
	let mut child = Command::new(exe)
		.arg(SUBCOMMAND)
		.arg(kind.arg())
		.env_clear()
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::null())
		.kill_on_drop(true)
		.spawn()
		.map_err(|error| {
			tracing::error!(%error, "could not start the text extraction process");
			ExtractError::ExtractFailed
		})?;
	let (Some(mut stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
		return Err(ExtractError::ExtractFailed);
	};
	let work = async {
		let write = async {
			let _ = stdin.write_all(bytes).await;
			drop(stdin);
		};
		let mut output = Vec::new();
		let mut limited = stdout.take(MAX_TEXT_BYTES as u64 + 1);
		let read = limited.read_to_end(&mut output);
		let ((), read) = tokio::join!(write, read);
		read.map_err(|_| ExtractError::ExtractFailed)?;
		if output.len() > MAX_TEXT_BYTES {
			return Err(ExtractError::TextTooLong);
		}
		let status = child.wait().await.map_err(|_| ExtractError::ExtractFailed)?;
		if !status.success() {
			return Err(ExtractError::ExtractFailed);
		}
		String::from_utf8(output).map_err(|_| ExtractError::ExtractFailed)
	};
	tokio::time::timeout(TIMEOUT, work).await.map_err(|_| ExtractError::TimedOut)?
}

/// Entry point of the extraction child process: reads the file from stdin and writes its text to stdout.
#[must_use]
pub fn child_main(kind: Option<&str>) -> i32 {
	limit_memory();
	let Some(kind) = kind.and_then(DocumentKind::from_arg) else {
		return 2;
	};
	let mut input = Vec::new();
	if std::io::stdin().take(MAX_UPLOAD_BYTES as u64 + 1).read_to_end(&mut input).is_err() || input.len() > MAX_UPLOAD_BYTES {
		return 1;
	}
	let Some(text) = parse(kind, &input) else {
		return 1;
	};
	match std::io::stdout().lock().write_all(text.as_bytes()) {
		Ok(()) => 0,
		Err(_) => 1,
	}
}

/// Parses a file in the current process; only the child process calls this for HTML and PDF.
#[must_use]
pub fn parse(kind: DocumentKind, input: &[u8]) -> Option<String> {
	match kind {
		DocumentKind::Text => decode_text(input).ok(),
		DocumentKind::Html => html2text::from_read(input, HTML_WIDTH).ok(),
		DocumentKind::Pdf => {
			let pages = pdf_extract::extract_text_from_mem_by_pages(input).ok()?;
			let total = pdf_extract::Document::load_mem(input).map_or(pages.len(), |doc| doc.get_pages().len());
			let mut marked: Vec<String> = pages
				.iter()
				.enumerate()
				.map(|(index, page)| format!("<!-- page {} -->\n{}", index + 1, page.trim()))
				.collect();
			if total > pages.len() {
				marked.push(format!("<!-- pages {} to {total} could not be read -->", pages.len() + 1));
			}
			Some(marked.join("\n\n"))
		}
	}
}

#[cfg(target_os = "linux")]
#[allow(unsafe_code)]
fn limit_memory() {
	let limit = libc::rlimit {
		rlim_cur: MEMORY_LIMIT_BYTES,
		rlim_max: MEMORY_LIMIT_BYTES,
	};
	unsafe {
		libc::setrlimit(libc::RLIMIT_AS, &raw const limit);
	}
}

#[cfg(not(target_os = "linux"))]
fn limit_memory() {}
