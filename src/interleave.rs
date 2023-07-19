use pin_project_lite::pin_project;

use futures::stream::{Stream};

use std::pin::Pin;
use std::task::{Context, Poll};

pin_project! {
    /// Simple stream combinator to interleave two streams (in no particular order)
    #[must_use = "streams do nothing unless polled"]
    pub struct Interleave<L: Stream, R: Stream> {
        #[pin]
        left: L,
        #[pin]
        right: R
    }
}

impl<I, L: Stream<Item=I>, R: Stream<Item=I>> Interleave<L, R> {
    /// Probably a good idea to always 'fuse' input streams (ie, sure they can be polled after finishing)
    pub fn new(left: L, right: R) -> Interleave<L, R> {
        Interleave { left, right }
    }
}

impl<I, L: Stream<Item=I>, R: Stream<Item=I>> Stream for Interleave<L, R> {
    type Item = I;

    fn poll_next(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();

        match Stream::poll_next(this.left, ctx) {
            Poll::Ready(Some(line)) => {
                Poll::Ready(Some(line)) 
            }
            Poll::Ready(None) |
            Poll::Pending => {
                Stream::poll_next(this.right, ctx)
            }
        }
    }
} 
