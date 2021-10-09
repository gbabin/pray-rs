extern crate png;

use std::fs::File;
use std::io::BufReader;
use std::io::BufWriter;
use std::io::prelude::*;
use std::net::TcpListener;
use std::net::TcpStream;
use std::path::Path;
use std::str;
use std::thread;
use std::time;

const GROUP_SIZE : u32 = 64;
const GROUPS_COUNT : u32 = 2;

const WINDOW_WIDTH : u32 = GROUP_SIZE * GROUPS_COUNT;
const WINDOW_HEIGHT : u32 = (WINDOW_WIDTH * 9) / 16;

const XML_FILE : &str = "../scenes/testScene1.xml";

fn main() {
    let listener = TcpListener::bind("127.0.0.1:1234").unwrap();

    println!("Listening...");

    for stream in listener.incoming() {
        let stream = stream.unwrap();
        println!("Connection established");
        handle_connection(stream);
    }
}

fn set_pixel(image: &mut [u8], x: u32, y: u32, r: u8, g: u8, b: u8) {
    image[(3*(x + WINDOW_WIDTH*(WINDOW_HEIGHT-1-y))    ) as usize] = r;
    image[(3*(x + WINDOW_WIDTH*(WINDOW_HEIGHT-1-y)) + 1) as usize] = g;
    image[(3*(x + WINDOW_WIDTH*(WINDOW_HEIGHT-1-y)) + 2) as usize] = b;
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

fn receive_command_result(reader: &mut BufReader<TcpStream>, image: &mut [u8], y: u32) {
    let mut size_bytes : Vec<u8> = vec![];
    reader.read_until(' ' as u8, &mut size_bytes).unwrap();
    size_bytes.truncate(size_bytes.len() - 1);
    let size : usize = str::from_utf8(&size_bytes).unwrap().parse::<usize>().unwrap();
    println!("size = {}", size);

    let mut data : Vec<u8> = vec![0; size+1];
    reader.read_exact(&mut data).unwrap();
    println!("data = <{}>", String::from_utf8_lossy(&data));
    println!("data = <{:?}>", data);

    for x in 0 .. WINDOW_WIDTH {
        set_pixel(image, x, y,
                  data[(9+3*x  ) as usize],
                  data[(9+3*x+1) as usize],
                  data[(9+3*x+2) as usize]);
    }
}

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

    let path = Path::new("image.png");
    let file = File::create(path).unwrap();
    let ref mut w = BufWriter::new(file);
    let mut encoder = png::Encoder::new(w, WINDOW_WIDTH, WINDOW_HEIGHT);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer_png = encoder.write_header().unwrap();
    let mut image_data = [127u8; (WINDOW_WIDTH*WINDOW_HEIGHT*3) as usize];

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

    for y in 0 .. WINDOW_HEIGHT {
        thread::sleep(second);

        println!(">>> Sending CALCULATE ({}/{}) ...", y+1, WINDOW_HEIGHT);
        let command_info = format!("CALCULATE {} {} {} {}", 1, WINDOW_HEIGHT-1-y, WINDOW_WIDTH, 1);
        send_command(&mut writer, &command_info);

        println!(">>> Waiting CALCULATING ...");
        receive_command(&mut reader);

        println!(">>> Waiting RESULT ...");
        receive_command_result(&mut reader, &mut image_data, y);
    }

    println!(">>> Saving image ...");
    writer_png.write_image_data(&image_data).unwrap();
}
