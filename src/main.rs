use std::io::prelude::*;
use std::io::BufReader;
use std::net::TcpListener;
use std::net::TcpStream;
use std::str;

fn main() {
    let listener = TcpListener::bind("127.0.0.1:1234").unwrap();

    println!("Listening...");

    for stream in listener.incoming() {
        let stream = stream.unwrap();
        println!("Connection established!");
        handle_connection(stream);
    }
}

fn handle_connection(stream: TcpStream) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());

    let mut size_bytes : Vec<u8> = vec![];
    reader.read_until(' ' as u8, &mut size_bytes).unwrap();
    size_bytes.truncate(size_bytes.len() - 1);
    let size : usize = str::from_utf8(&size_bytes).unwrap().parse::<usize>().unwrap();
    println!("size = {}", size);

    let mut command : Vec<u8> = vec![];
    reader.read_until(' ' as u8, &mut command).unwrap();
    command.retain(|&x| x != ' ' as u8 && x != 0);
    let command_len = command.len();
    println!("command = <{}>", String::from_utf8(command).unwrap());

    let mut data : Vec<u8> = vec![0; size-command_len];
    reader.read_exact(&mut data).unwrap();
    println!("data = <{}>", String::from_utf8(data).unwrap());
}
