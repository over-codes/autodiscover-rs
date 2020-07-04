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
    let listener = TcpListener::bind("0.0.0.0:0")?;
    let socket = listener.local_addr()?;
    thread::spawn(move || {
            autodiscover_rs::run(&socket, Method::Broadcast("255.255.255.255:2020".parse().unwrap()), |s| {
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