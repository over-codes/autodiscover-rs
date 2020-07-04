# autodiscover-rs

autodiscover-rs implements a simple algorithm to detect peers on an
IP network, connects to them, and calls back with the connected stream. The algorthm supports both UDP broadcast and multicasting.

## Usage

Cargo.toml
```
autodiscover-rs = "0.1.0"
```

In your app:

```
use std::net::{TcpListener, TcpStream};
use std::thread;

use autodiscover_rs::{self, Method};
use env_logger;

fn handle_client(stream: std::io::Result<TcpStream>) {
    println!("Got a connection from {:?}", stream.unwrap().peer_addr());
}

fn main() -> std::io::Result<()> {
    env_logger::init();
    // make sure to bind before announcing ready
    let listener = TcpListener::bind(":::0")?;
    let socket = listener.local_addr()?;
    thread::spawn(move || {
            autodiscover_rs::run(&socket, Method::Multicast("[ff0e::1]:2000".parse().unwrap()), |s| {
                // change this to be async if using tokio or async_std
                thread::spawn(|| handle_client(s));
        }).unwrap();
    });
    let mut incoming = listener.incoming();
    while let Some(stream) = incoming.next() {
        thread::spawn(|| handle_client(stream));
    }
    Ok(())
}
```

## Notes

By default, discover will spawn one thread per connection. This is not ideal, but is the only way to ensure we don't block on a bad client. If you use async, the cost of this is much lower.

The algorithm for peer discovery is to:
- Send a message to the broadcast/multicast address with the configured 'listen address' compressed to a 6 byte (IPv4) or 18 byte (IPv6) packet
- Start listening for new messages on the broadcast/multicast address; when one is recv., connect to it and run the callback

This has a few gotchas:
- If a broadcast packet goes missing, some connections won't be made


### Packet format

The IP address we are broadcasting is of the form:

IPv4:

    buff[0..4].clone_from_slice(&addr.ip().octets());
    buff[4..6].clone_from_slice(&addr.port().to_be_bytes());

IPv6:

    buff[0..16].clone_from_slice(&addr.ip().octets());
    buff[16..18].clone_from_slice(&addr.port().to_be_bytes());
