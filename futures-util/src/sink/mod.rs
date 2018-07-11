//! Sinks
//!
//! This module contains a number of functions for working with `Sink`s,
//! including the `SinkExt` trait which adds methods to `Sink` types.

use core::marker::Unpin;
use either::Either;
use futures_core::future::Future;
use futures_core::stream::Stream;
use futures_sink::Sink;

mod close;
pub use self::close::Close;

mod fanout;
pub use self::fanout::Fanout;

mod flush;
pub use self::flush::Flush;

mod err_into;
pub use self::err_into::SinkErrInto;

mod map_err;
pub use self::map_err::SinkMapErr;

mod send;
pub use self::send::Send;

mod send_all;
pub use self::send_all::SendAll;

mod with;
pub use self::with::With;

mod with_flat_map;
pub use self::with_flat_map::WithFlatMap;

if_std! {
    mod buffer;
    pub use self::buffer::Buffer;
}

impl<T: ?Sized> SinkExt for T where T: Sink {}

/// An extension trait for `Sink`s that provides a variety of convenient
/// combinator functions.
pub trait SinkExt: Sink {
    /// Composes a function *in front of* the sink.
    ///
    /// This adapter produces a new sink that passes each value through the
    /// given function `f` before sending it to `self`.
    ///
    /// To process each value, `f` produces a *future*, which is then polled to
    /// completion before passing its result down to the underlying sink. If the
    /// future produces an error, that error is returned by the new sink.
    ///
    /// Note that this function consumes the given sink, returning a wrapped
    /// version, much like `Iterator::map`.
    fn with<U, Fut, F, E>(self, f: F) -> With<Self, U, Fut, F>
        where F: FnMut(U) -> Fut,
              Fut: Future<Output = Result<Self::SinkItem, E>>,
              E: From<Self::SinkError>,
              Self: Sized
    {
        With::new(self, f)
    }

    /// Composes a function *in front of* the sink.
    ///
    /// This adapter produces a new sink that passes each value through the
    /// given function `f` before sending it to `self`.
    ///
    /// To process each value, `f` produces a *stream*, of which each value
    /// is passed to the underlying sink. A new value will not be accepted until
    /// the stream has been drained
    ///
    /// Note that this function consumes the given sink, returning a wrapped
    /// version, much like `Iterator::flat_map`.
    ///
    /// # Examples
    ///
    /// ```
    /// # extern crate futures;
    /// use futures::prelude::*;
    /// use futures::channel::mpsc;
    /// use futures::executor::block_on;
    /// use std::collections::VecDeque;
    ///
    /// # fn main() {
    /// let (mut tx, rx) = mpsc::channel(5);
    ///
    /// let mut tx = tx.with_flat_map(|x| {
    ///     VecDeque::from(vec![Ok(42); x])
    /// });
    ///
    /// block_on(tx.send(5)).unwrap();
    /// drop(tx);
    /// let received: Vec<i32> = block_on(rx.collect());
    /// assert_eq!(received, vec![42, 42, 42, 42, 42]);
    /// # }
    /// ```
    fn with_flat_map<U, St, F>(self, f: F) -> WithFlatMap<Self, U, St, F>
        where F: FnMut(U) -> St,
              St: Stream<Item = Result<Self::SinkItem, Self::SinkError>>,
              Self: Sized
    {
        WithFlatMap::new(self, f)
    }

    /*
    fn with_map<U, F>(self, f: F) -> WithMap<Self, U, F>
        where F: FnMut(U) -> Self::SinkItem,
              Self: Sized;

    fn with_filter<F>(self, f: F) -> WithFilter<Self, F>
        where F: FnMut(Self::SinkItem) -> bool,
              Self: Sized;

    fn with_filter_map<U, F>(self, f: F) -> WithFilterMap<Self, U, F>
        where F: FnMut(U) -> Option<Self::SinkItem>,
              Self: Sized;
     */

    /// Transforms the error returned by the sink.
    fn sink_map_err<E, F>(self, f: F) -> SinkMapErr<Self, F>
        where F: FnOnce(Self::SinkError) -> E,
              Self: Sized,
    {
        SinkMapErr::new(self, f)
    }

    /// Map this sink's error to a different error type using the `Into` trait.
    ///
    /// If wanting to map errors of a `Sink + Stream`, use `.sink_err_into().err_into()`.
    fn sink_err_into<E>(self) -> err_into::SinkErrInto<Self, E>
        where Self: Sized,
              Self::SinkError: Into<E>,
    {
        SinkErrInto::new(self)
    }


    /// Adds a fixed-size buffer to the current sink.
    ///
    /// The resulting sink will buffer up to `capacity` items when the
    /// underlying sink is unwilling to accept additional items. Calling `flush`
    /// on the buffered sink will attempt to both empty the buffer and complete
    /// processing on the underlying sink.
    ///
    /// Note that this function consumes the given sink, returning a wrapped
    /// version, much like `Iterator::map`.
    ///
    /// This method is only available when the `std` feature of this
    /// library is activated, and it is activated by default.
    #[cfg(feature = "std")]
    fn buffer(self, capacity: usize) -> Buffer<Self>
        where Self: Sized,
    {
        Buffer::new(self, capacity)
    }

    /// Close the sink.
    fn close<'a>(&'a mut self) -> Close<'a, Self>
        where Self: Unpin,
    {
        Close::new(self)
    }

    /// Fanout items to multiple sinks.
    ///
    /// This adapter clones each incoming item and forwards it to both this as well as
    /// the other sink at the same time.
    fn fanout<Si>(self, other: Si) -> Fanout<Self, Si>
        where Self: Sized,
              Self::SinkItem: Clone,
              Si: Sink<SinkItem=Self::SinkItem, SinkError=Self::SinkError>
    {
        Fanout::new(self, other)
    }

    /// Flush the sync, processing all pending items.
    ///
    /// This adapter is intended to be used when you want to stop sending to the sink
    /// until all current requests are processed.
    fn flush<'a>(&'a mut self) -> Flush<'a, Self>
        where Self: Unpin,
    {
        Flush::new(self)
    }

    /// A future that completes after the given item has been fully processed
    /// into the sink, including flushing.
    ///
    /// Note that, **because of the flushing requirement, it is usually better
    /// to batch together items to send via `send_all`, rather than flushing
    /// between each item.**
    fn send<'a>(&'a mut self, item: Self::SinkItem) -> Send<'a, Self>
        where Self: Unpin,
    {
        Send::new(self, item)
    }

    /// A future that completes after the given stream has been fully processed
    /// into the sink, including flushing.
    ///
    /// This future will drive the stream to keep producing items until it is
    /// exhausted, sending each item to the sink. It will complete once both the
    /// stream is exhausted, the sink has received all items, and the sink has
    /// been flushed. Note that the sink is **not** closed.
    ///
    /// Doing `sink.send_all(stream)` is roughly equivalent to
    /// `stream.forward(sink)`. The returned future will exhaust all items from
    /// `stream` and send them to `self`.
    fn send_all<'a, St>(
        &'a mut self,
        stream: &'a mut St
    ) -> SendAll<'a, Self, St>
        where St: Stream<Item = Self::SinkItem> + Unpin,
              Self: Unpin,
    {
        SendAll::new(self, stream)
    }

    /// Wrap this sink in an `Either` sink, making it the left-hand variant
    /// of that `Either`.
    ///
    /// This can be used in combination with the `right_sink` method to write `if`
    /// statements that evaluate to different streams in different branches.
    fn left_sink<Si2>(self) -> Either<Self, Si2>
        where Si2: Sink<SinkItem = Self::SinkItem, SinkError = Self::SinkError>,
              Self: Sized
    {
        Either::Left(self)
    }

    /// Wrap this stream in an `Either` stream, making it the right-hand variant
    /// of that `Either`.
    ///
    /// This can be used in combination with the `left_sink` method to write `if`
    /// statements that evaluate to different streams in different branches.
    fn right_sink<Si1>(self) -> Either<Si1, Self>
        where Si1: Sink<SinkItem = Self::SinkItem, SinkError = Self::SinkError>,
              Self: Sized
    {
        Either::Right(self)
    }
}
