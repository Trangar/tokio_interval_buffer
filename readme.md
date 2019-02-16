Provides an `IntervalBuffer` that can be used to buffer a stream and emit the values at a regular interval.

This is useful for when you receive streaming data but want to parse it in bulk.

```rust
extern crate irc;
extern crate failure;
extern crate tokio_interval_buffer;
extern crate futures;
extern crate tokio;

use irc::client::prelude::*;
use tokio_interval_buffer::IntervalBuffer;

fn main() {
    tokio::run(futures::future::lazy(|| {
        let client = IrcClient::from_config(Config {
            nickname: Some(String::from("...")),
            server: Some(String::from("...")),
            channels: Some(vec![String::from("...")]),
            ..Default::default()
        })
        .expect("Could not create an irc client");

        // Take the IRC stream and process all the messages every 10 seconds
        let buffered_receiver = IntervalBuffer::<_, _, failure::Error>::new(
            client
                .stream()
                .map_err(|e| failure::format_err!("Client stream error: {:?}", e)),
            std::time::Duration::from_secs(10),
        );

        buffered_receiver
            .for_each(|b| {
                println!("Buffer: {:?}", b);
                Ok(())
            })
            .map_err(|e| {
                println!("Buffered receiver error: {:?}", e);
            })
    }));
}
```
