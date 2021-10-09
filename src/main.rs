use std::io::prelude::*;
use std::io::BufReader;
use std::io::BufWriter;
use std::net::TcpListener;
use std::net::TcpStream;
use std::str;
use std::{thread, time};

fn main() {
    let listener = TcpListener::bind("127.0.0.1:1234").unwrap();

    println!("Listening...");

    for stream in listener.incoming() {
        let stream = stream.unwrap();
        println!("Connection established");
        handle_connection(stream);
    }
}

fn receive_command(reader: &mut BufReader<TcpStream>) {
    let mut size_bytes : Vec<u8> = vec![];
    reader.read_until(' ' as u8, &mut size_bytes).unwrap();
    size_bytes.truncate(size_bytes.len() - 1);
    let size : usize = str::from_utf8(&size_bytes).unwrap().parse::<usize>().unwrap();
    println!("size = {}", size);

    let mut data : Vec<u8> = vec![0; size+1];
    reader.read_exact(&mut data).unwrap();
    println!("data = <{}>", String::from_utf8_lossy(&data));
    println!("data = <{:?}>", data); 
}

static WINDOW_WIDTH : u32 = 800;
static WINDOW_HEIGHT : u32 = 600;

static XML_FILE : &str = "../scenes/bille.xml";

fn send_command(writer: &mut BufWriter<TcpStream>, command: &str) {
    let command_bytes = command.as_bytes();
    write!(writer, "{} ", command_bytes.len()).unwrap();
    writer.write_all(command_bytes).unwrap();
    writer.write(&vec![0]).unwrap();
    writer.flush().unwrap();
}

fn handle_connection(stream: TcpStream) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut writer = BufWriter::new(stream);

    let second = time::Duration::from_secs(1);

    println!(">>> Waiting LOGIN ...");
    receive_command(&mut reader);

    thread::sleep(second);

    println!(">>> Sending INFO ...");
    let command_info = format!("INFO {} {}", WINDOW_WIDTH, WINDOW_HEIGHT);
    send_command(&mut writer, &command_info);

    thread::sleep(second);

    println!(">>> Sending SETSCENE ...");
    let command_info = format!("SETSCENE {}", XML_FILE);
    send_command(&mut writer, &command_info);

    thread::sleep(second);

    println!(">>> Sending CALCULATE ...");
    let command_info = format!("CALCULATE {} {} {} {}", 1, 0, 1, 1);
    send_command(&mut writer, &command_info);


    println!(">>> Waiting CALCULATING ...");
    receive_command(&mut reader);

    println!(">>> Waiting RESULT ...");
    receive_command(&mut reader);
}
