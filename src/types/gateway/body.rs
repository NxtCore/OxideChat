use axum::body::{Body, Bytes, HttpBody};
use std::pin::Pin;
use std::task::{Context, Poll};

pub(crate) struct ReservedBody<T> {
	body: Body,
	reservation: Option<T>,
}

impl<T> ReservedBody<T> {
	pub(crate) fn new(body: Body, reservation: T) -> Self {
		Self {
			body,
			reservation: Some(reservation),
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
