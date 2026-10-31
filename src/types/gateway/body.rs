use crate::types::RequestLogAbort;
use axum::body::{Body, Bytes, HttpBody};
use std::pin::Pin;
use std::task::{Context, Poll};

pub(crate) struct ReservedBody<T> {
	body: Body,
	reservation: Option<T>,
	abort_log: Option<RequestLogAbort>,
}

impl<T> ReservedBody<T> {
	pub(crate) fn new(body: Body, reservation: T) -> Self {
		Self {
			body,
			reservation: Some(reservation),
			abort_log: None,
		}
	}

	pub(crate) fn with_abort_log(mut self, abort_log: RequestLogAbort) -> Self {
		if !self.body.is_end_stream() {
			self.abort_log = Some(abort_log);
		}
		self
	}
}

impl<T> Drop for ReservedBody<T> {
	fn drop(&mut self) {
		if let Some(abort_log) = self.abort_log.take() {
			abort_log.record_in_background();
		}
	}
}

impl<T: Send + Unpin + 'static> HttpBody for ReservedBody<T> {
	type Data = Bytes;
	type Error = axum::Error;

	fn poll_frame(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
		let result = Pin::new(&mut self.body).poll_frame(context);
		if matches!(result, Poll::Ready(None | Some(Err(_)))) || self.body.is_end_stream() {
			self.reservation.take();
			self.abort_log.take();
		}
		result
	}

	fn is_end_stream(&self) -> bool {
		self.body.is_end_stream()
	}

	fn size_hint(&self) -> http_body::SizeHint {
		self.body.size_hint()
	}
}
