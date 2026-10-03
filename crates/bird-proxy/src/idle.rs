use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use std::time::Duration;

use bytes::Bytes;
use hyper::body::{Body, Frame, SizeHint};
use tokio::time::{Instant, Sleep};

use crate::body::BoxError;

// a stalled upstream would otherwise hold the client connection and its slot forever
pub(crate) struct IdleTimeout<B> {
    inner: B,
    limit: Duration,
    last_frame: Instant,
    deadline: Pin<Box<Sleep>>,
}

impl<B> IdleTimeout<B> {
    pub(crate) fn new(inner: B, limit: Duration) -> Self {
        Self {
            inner,
            limit,
            last_frame: Instant::now(),
            deadline: Box::pin(tokio::time::sleep(limit)),
        }
    }
}

impl<B> Body for IdleTimeout<B>
where
    B: Body<Data = Bytes> + Unpin,
    B::Error: Into<BoxError>,
{
    type Data = Bytes;
    type Error = BoxError;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, BoxError>>> {
        let this = self.get_mut();
        if let Poll::Ready(frame) = Pin::new(&mut this.inner).poll_frame(cx) {
            // the timer is only re-armed when it fires, keeping per-frame work to a clock read
            this.last_frame = Instant::now();
            return Poll::Ready(frame.map(|frame| frame.map_err(Into::into)));
        }
        loop {
            ready!(this.deadline.as_mut().poll(cx));
            let next = this.last_frame + this.limit;
            if next <= Instant::now() {
                return Poll::Ready(Some(Err("upstream stopped sending the response".into())));
            }
            this.deadline.as_mut().reset(next);
        }
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use http_body_util::{BodyExt, Full};

    use super::*;

    struct Stalled;

    impl Body for Stalled {
        type Data = Bytes;
        type Error = Infallible;

        fn poll_frame(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
        ) -> Poll<Option<Result<Frame<Bytes>, Infallible>>> {
            Poll::Pending
        }
    }

    #[tokio::test]
    async fn passes_frames_through() {
        let body = IdleTimeout::new(Full::new(Bytes::from_static(b"hi")), Duration::from_secs(1));
        assert_eq!(body.collect().await.unwrap().to_bytes(), "hi");
    }

    #[tokio::test]
    async fn fails_when_the_body_stalls() {
        let mut body = IdleTimeout::new(Stalled, Duration::from_millis(50));
        assert!(body.frame().await.unwrap().is_err());
    }
}
