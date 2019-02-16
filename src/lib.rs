//! Provides an `IntervalBuffer` that can be used to buffer a stream and emit the values at a regular interval.
//!
//! This is useful for when you receive streaming data but want to parse it in bulk.
//!
//! ```rust
//! extern crate irc;
//! extern crate failure;
//! extern crate tokio_interval_buffer;
//! extern crate futures;
//! extern crate tokio;
//!
//! use irc::client::prelude::*;
//! use tokio_interval_buffer::IntervalBuffer;
//!
//! fn main() {
//!     tokio::run(futures::future::lazy(|| {
//!         let client = IrcClient::from_config(Config {
//!             nickname: Some(String::from("...")),
//!             server: Some(String::from("...")),
//!             channels: Some(vec![String::from("...")]),
//!             ..Default::default()
//!         })
//!         .expect("Could not create an irc client");
//!
//!         // Take the IRC stream and process all the messages every 10 seconds
//!         let buffered_receiver = IntervalBuffer::<_, _, failure::Error>::new(
//!             client
//!                 .stream()
//!                 .map_err(|e| failure::format_err!("Client stream error: {:?}", e)),
//!             std::time::Duration::from_secs(10),
//!         );
//!
//!         buffered_receiver
//!             .for_each(|b| {
//!                 println!("Buffer: {:?}", b);
//!                 Ok(())
//!             })
//!             .map_err(|e| {
//!                 println!("Buffered receiver error: {:?}", e);
//!             })
//!     }));
//! }
//! ```

use futures::prelude::*;
use std::time::Duration;
use tokio::timer::Interval;

/// This buffer takes a stream and an interval, and will emit a Vec<Stream::Item> every interval.
///
/// If no messages are send in that interval, the stream will not send anything.
/// If the initial stream ends, this stream ends as well.
///
/// If either the stream or the interval timer emits an error, this stream will emit the same error.
/// For the timer, `From<tokio::timer::Error>` has to be emitted for the error.
/// In the future I want to be able to configure a `.map_err` function for this.
pub struct IntervalBuffer<Stream, Item, Error, Container: Insertable<Item> = Vec<Item>>
where
    Stream: futures::Stream<Item = Item, Error = Error>,
    Error: From<tokio::timer::Error>,
{
    stream: Stream,
    timer: Interval,
    buffer: Container,
}

/// A generic component used for the container for the IntervalBuffer.
/// This can be implemented for any type, and is only implemented for Vec<T> by default.
///
/// For performance-specific implementations this should be replaced for whatever works for your situation.
pub trait Insertable<T>: Sized {
    /// Insert an item in the buffer
    fn insert(&mut self, t: T);

    /// Return the current content of the buffer, and clear itself.
    ///
    /// If the container is empty, you can return None.
    ///
    /// For Vec<T> this is implemented as `std::mem::replace(self, Vec::new())`
    fn return_content_and_clear(&mut self) -> Option<Self>;
}

impl<T> Insertable<T> for Vec<T> {
    fn insert(&mut self, t: T) {
        self.push(t);
    }

    fn return_content_and_clear(&mut self) -> Option<Vec<T>> {
        if self.is_empty() {
            None
        } else {
            // Make sure to preserve the capacity so we have a decent estimate of how big the buffer should be.
            // TODO: Maybe keep a history of the last 5 capacities and take the average of this? Currently this will stay the largest size the application has seen.
            let new_vec = Vec::with_capacity(self.capacity());
            Some(std::mem::replace(self, new_vec))
        }
    }
}

impl<Stream, Item, Error, Container> IntervalBuffer<Stream, Item, Error, Container>
where
    Stream: futures::Stream<Item = Item, Error = Error>,
    Error: From<tokio::timer::Error>,
    Container: Insertable<Item>,
{
    /// Create a new IntervalBuffer with a default container. This will simply call `new_with_container(.., .., Container::default())`. See that function for more informaiton.
    pub fn new(stream: Stream, interval: Duration) -> Self
    where
        Container: Default,
    {
        Self::new_with_container(stream, interval, Container::default())
    }

    /// Create a new IntervalBuffer with a given stream, interval and container.
    ///
    /// The first time this stream will be able to send data is after `interval: Duration`. This will not emit immediately.
    ///
    /// If either the stream or the internal timer emits an error, this stream will emit an error.
    ///
    /// If the stream ends (by returning `Ok(Async::Ready(None))`), this stream will immediately return. The internal timer will not be polled.
    pub fn new_with_container(stream: Stream, interval: Duration, container: Container) -> Self {
        let timer = Interval::new_interval(interval);
        IntervalBuffer {
            stream,
            timer,
            buffer: container,
        }
    }

    /// Get a reference to the internal buffer
    pub fn buffer(&self) -> &Container {
        &self.buffer
    }

    /// Get a mutable reference to the internal buffer.
    ///
    /// This can be used to e.g. set the capacity for a Vec.
    pub fn buffer_mut(&mut self) -> &mut Container {
        &mut self.buffer
    }
}

impl<Stream, Item, Error, Container> futures::Stream
    for IntervalBuffer<Stream, Item, Error, Container>
where
    Stream: futures::Stream<Item = Item, Error = Error>,
    Error: From<tokio::timer::Error>,
    Container: Insertable<Item>,
{
    type Item = Container;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        loop {
            match self.stream.poll()? {
                Async::NotReady => break,
                Async::Ready(Some(v)) => self.buffer.insert(v),
                Async::Ready(None) => return Ok(Async::Ready(None)),
            }
        }

        if let Async::Ready(_) = self.timer.poll()? {
            let result = self.buffer.return_content_and_clear();
            if result.is_some() {
                Ok(Async::Ready(result))
            } else {
                Ok(Async::NotReady)
            }
        } else {
            Ok(Async::NotReady)
        }
    }
}
