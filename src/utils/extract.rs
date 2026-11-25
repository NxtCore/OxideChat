//! Text extraction for files attached to chat messages.
//!
//! HTML and PDF parsers run in a child process (this binary with the `extract` subcommand) with a time
//! and memory limit and no environment, so a hanging parser or a compression bomb is killed instead of
//! taking the server down.

use crate::types::chat::AttachmentError;
use std::io::{Read, Write};
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

/// Largest file a user may attach.
pub const MAX_UPLOAD_BYTES: usize = 25 * 1024 * 1024;

/// Largest text an attachment may yield.
pub const MAX_TEXT_BYTES: usize = 8 * 1024 * 1024;

/// The command-line argument that starts the extraction child process.
pub const SUBCOMMAND: &str = "extract";

const TIMEOUT: Duration = Duration::from_secs(30);

#[cfg(target_os = "linux")]
const MEMORY_LIMIT_BYTES: libc::rlim_t = 1024 * 1024 * 1024;

const HTML_WIDTH: usize = 1000;

const TEXT_EXTENSIONS: &[&str] = &[
	"txt", "md", "markdown", "csv", "tsv", "json", "jsonl", "yaml", "yml", "xml", "log", "ini", "toml", "sql", "rs", "py", "js", "mjs", "ts", "tsx", "jsx", "vue", "go",
	"java", "kt", "c", "h", "cpp", "hpp", "cs", "rb", "php", "sh", "css", "scss", "swift",
];

/// How a file is turned into text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentKind {
	Text,
	Html,
	Pdf,
}

impl DocumentKind {
	/// The kind of a file from its extension; a PDF must also start like one.
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

	/// The media type the attachment is stored with.
	#[must_use]
	pub const fn mime_type(self) -> &'static str {
		match self {
			Self::Text => "text/plain",
			Self::Html => "text/html",
			Self::Pdf => "application/pdf",
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
/// Returns an [`AttachmentError`] when the file is not text, cannot be parsed in time, or yields no or too much text.
pub async fn extract(kind: DocumentKind, bytes: &[u8]) -> Result<String, AttachmentError> {
	let text = match kind {
		DocumentKind::Text => decode_text(bytes)?,
		DocumentKind::Html | DocumentKind::Pdf => run_child(kind, bytes).await?,
	};
	let text = text.trim();
	if text.is_empty() {
		return Err(AttachmentError::Empty);
	}
	if text.len() > MAX_TEXT_BYTES {
		return Err(AttachmentError::TextTooLong);
	}
	Ok(text.to_owned())
}

fn decode_text(bytes: &[u8]) -> Result<String, AttachmentError> {
	let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
	if bytes.contains(&0) {
		return Err(AttachmentError::NotText);
	}
	String::from_utf8(bytes.to_vec()).map_err(|_| AttachmentError::NotText)
}

async fn run_child(kind: DocumentKind, bytes: &[u8]) -> Result<String, AttachmentError> {
	let exe = std::env::current_exe().map_err(|error| {
		tracing::error!(%error, "could not find the server binary for text extraction");
		AttachmentError::ExtractFailed
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
			AttachmentError::ExtractFailed
		})?;
	let (Some(mut stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
		return Err(AttachmentError::ExtractFailed);
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
		read.map_err(|_| AttachmentError::ExtractFailed)?;
		if output.len() > MAX_TEXT_BYTES {
			return Err(AttachmentError::TextTooLong);
		}
		let status = child.wait().await.map_err(|_| AttachmentError::ExtractFailed)?;
		if !status.success() {
			return Err(AttachmentError::ExtractFailed);
		}
		String::from_utf8(output).map_err(|_| AttachmentError::ExtractFailed)
	};
	tokio::time::timeout(TIMEOUT, work).await.map_err(|_| AttachmentError::TimedOut)?
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
			let marked: Vec<String> = pages
				.iter()
				.enumerate()
				.map(|(index, page)| format!("<!-- page {} -->\n{}", index + 1, page.trim()))
				.collect();
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
