//! autodiscovery-rs provides a function to automatically detect and connect to peers.
//! 
//! # Examples
//! 
//! ```rust,no_run
//! use std::net::{TcpListener, TcpStream};
//! use std::thread;
//! use autodiscover_rs::Method;
//! use env_logger;
//!
//! fn handle_client(stream: std::io::Result<TcpStream>) {
//!     println!("Got a connection from {:?}", stream.unwrap().peer_addr());
//! }
//!
//! fn main() -> std::io::Result<()> {
//!     env_logger::init();
//!     // make sure to bind before announcing ready
//!     let listener = TcpListener::bind(":::0")?;
//!     // get the port we were bound too; note that the trailing :0 above gives us a random unused port
//!     let socket = listener.local_addr()?;
//!     thread::spawn(move || {
//!         // this function blocks forever; running it a seperate thread
//!         autodiscover_rs::run(&socket, Method::Multicast("[ff0e::1]:1337".parse().unwrap()), |s| {
//!             // change this to task::spawn if using async_std or tokio
//!             thread::spawn(|| handle_client(s));
//!         }).unwrap();
//!     });
//!     let mut incoming = listener.incoming();
//!     while let Some(stream) = incoming.next() {
//!         // if you are using an async library, such as async_std or tokio, you can convert the stream to the
//!         // appropriate type before using task::spawn from your library of choice.
//!         thread::spawn(|| handle_client(stream));
//!     }
//!     Ok(())
//! }
//! ```
use std::convert::TryInto;
use std::net::{
    IpAddr,
    SocketAddr,
    TcpStream,
    UdpSocket,
    Ipv4Addr,
};
use socket2::{Socket, Domain, Type};
use log::{trace, warn};

/// Method describes whether a multicast or broadcast method for sending discovery messages should be used.
pub enum Method {
    /// Broadcast is an IPv4-only method of sending discovery messages; use a value such as `"255.255.255.255:1337".parse()` or
    /// `"192.168.0.255:1337".parse()` when using this method. The latter value will be specific to your network setup.
    Broadcast(SocketAddr),
    /// Multicast supports both IPv6 and IPv4 for sending discovery methods; use a value such as `"224.0.0.1".parse()` for IPv4, or
    /// `"[ff0e::1]:1337".parse()` for IPv6. To be frank, IPv6 confuses me, but that address worked on my machine.
    Multicast(SocketAddr),
}

fn handle_broadcast_message<F: Fn(std::io::Result<TcpStream>)>(socket: UdpSocket, my_socket: &SocketAddr, callback: &F) -> std::io::Result<()> {
    let mut buff = vec![0; 18];
    loop {
        let (bytes, _) = socket.recv_from(&mut buff)?;
        if let Ok(socket) = parse_bytes(bytes, &buff) {
            if socket == *my_socket {
                trace!("saw connection attempt from myself, this should happen once");
                continue;
            }
            let stream = TcpStream::connect(socket);
            callback(stream);
        }
    }
}

fn parse_bytes(len: usize, buff: &[u8]) -> Result<SocketAddr, ()> {
    let addr = match len {
        6 => {
            let ip = IpAddr::V4(u32::from_be_bytes(buff[0..4].try_into().unwrap()).into());
            let port = u16::from_be_bytes(buff[4..6].try_into().unwrap());
            SocketAddr::new(ip, port)
        },
        18 => {
            let ip: [u8; 16] = buff[0..16].try_into().unwrap();
            let ip = ip.into();
            let port = u16::from_be_bytes(buff[16..18].try_into().unwrap());
            SocketAddr::new(ip, port)
        },
        _ => {
            warn!("Dropping malformed packet; length was {}", len);
            return Err(())
        },
    };
    Ok(addr)
}

fn to_bytes(connect_to: &SocketAddr) -> Vec<u8> {
    match connect_to {
        SocketAddr::V6(addr) => {
            // length is 16 bytes + 2 bytes
            let mut buff = vec![0; 18];
            buff[0..16].clone_from_slice(&addr.ip().octets());
            buff[16..18].clone_from_slice(&addr.port().to_be_bytes());
            buff
        },
        SocketAddr::V4(addr) => {
            // length is 4 bytes + 2 bytes
            let mut buff = vec![0; 6];
            buff[0..4].clone_from_slice(&addr.ip().octets());
            buff[4..6].clone_from_slice(&addr.port().to_be_bytes());
            buff
        }
    }
}

/// run will block forever. It sends a notification using the configured method, then listens for other notifications and begins
/// connecting to them, calling spawn_callback (which should return right away!) with the connected streams. The connect_to address
/// should be a socket we have already bind'ed too, since we advertise that to other autodiscovery clients.
pub fn run<F: Fn(std::io::Result<TcpStream>)>(connect_to: &SocketAddr, method: Method, spawn_callback: F) -> std::io::Result<()> {
    match method {
        Method::Broadcast(addr) => {
            let socket = Socket::new(Domain::ipv4(), Type::dgram(), None)?;
            socket.set_reuse_address(true)?;
            socket.set_broadcast(true)?;
            socket.bind(&addr.into())?;
            let socket: UdpSocket = socket.into_udp_socket();
            socket.send_to(&to_bytes(connect_to), addr)?;
            handle_broadcast_message(socket, connect_to, &spawn_callback)?;
        },
        Method::Multicast(addr) => {
            let socket = Socket::new(Domain::ipv6(), Type::dgram(), None)?;
            socket.set_reuse_address(true)?;
            socket.bind(&addr.into())?;
            let socket: UdpSocket = socket.into_udp_socket();
            match addr.ip() {
                IpAddr::V4(addr) => {
                    let iface: Ipv4Addr = 0u32.into();
                    socket.join_multicast_v4(&addr, &iface)?;
                },
                IpAddr::V6(addr) => {
                    socket.join_multicast_v6(&addr, 0)?;
                },
            }
            // we need a different, temporary socket, to send multicast in IPv6
            {
                let socket = UdpSocket::bind(":::0")?;
                let result = socket.send_to(&to_bytes(connect_to), addr)?;
                warn!("sent {} bytes to {:?}", result, addr);
            }
            handle_broadcast_message(socket, connect_to, &spawn_callback)?;
        },
    }
    warn!("It looks like I stopped listening; this shouldn't happen.");
    Ok(())
}