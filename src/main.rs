use std::io::prelude::*;
use std::net::TcpListener;
use std::net::TcpStream;

fn main() {
    let listener = TcpListener::bind("127.0.0.1:1234").unwrap();

    println!("Listening...");

    for stream in listener.incoming() {
        let stream = stream.unwrap();
        println!("Connection established!");
        handle_connection(stream);
    }
}

fn handle_connection(mut stream: TcpStream) {
    let mut buffer : [u8; 1024] = [0; 1024];

    let n = stream.read(&mut buffer).unwrap();

    println!("n = {}", n);
    println!("buffer = <{}>", String::from_utf8_lossy(&buffer[..]));
}
