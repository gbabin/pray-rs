extern crate crossbeam_utils;
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
const GROUPS_COUNT : u32 = 60; // 3840 × 2160 px

const WINDOW_WIDTH : u32 = GROUP_SIZE * GROUPS_COUNT;
const WINDOW_HEIGHT : u32 = (WINDOW_WIDTH * 9) / 16;

const XML_FILE : &str = "../scenes/testScene1.xml";

const IMAGE_DATA_SIZE : usize = (WINDOW_WIDTH*WINDOW_HEIGHT*3) as usize;

const CLIENTS_COUNT : u32 = 6;

fn main() {
    let listener = TcpListener::bind("127.0.0.1:1234").unwrap();

    let path = Path::new("image.png");
    let file = File::create(path).unwrap();
    let ref mut w = BufWriter::new(file);
    let mut encoder = png::Encoder::new(w, WINDOW_WIDTH, WINDOW_HEIGHT);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer_png = encoder.write_header().unwrap();
    let mut image_data = vec![127u8; IMAGE_DATA_SIZE];

    println!("Listening...");

    let chunk_line_count = WINDOW_HEIGHT/CLIENTS_COUNT + u32::from(WINDOW_HEIGHT % CLIENTS_COUNT != 0);
    let chunk_size : usize = (chunk_line_count * WINDOW_WIDTH * 3) as usize;

    crossbeam_utils::thread::scope(|sc| {
        for (i, image_chunk) in image_data.chunks_mut(chunk_size).enumerate() {
            let id : u32 = (i+1) as u32;
            let (stream, _addr) = listener.accept().unwrap();

            println!(">>> [{}] Connection established", id);

            sc.spawn(move |_| {
                handle_connection(stream, id, image_chunk,
                                  (id-1)*chunk_line_count,
                                  u32::min(id*chunk_line_count-1, WINDOW_HEIGHT-1));
            });
        }
    }).unwrap();

    println!(">>> Saving image ...");
    writer_png.write_image_data(&image_data).unwrap();
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
    //println!("data = <{:?}>", data);
}

fn receive_command_result(reader: &mut BufReader<TcpStream>, image: &mut [u8], relative_y: u32) {
    let mut size_bytes : Vec<u8> = vec![];
    reader.read_until(' ' as u8, &mut size_bytes).unwrap();
    size_bytes.truncate(size_bytes.len() - 1);
    let size : usize = str::from_utf8(&size_bytes).unwrap().parse::<usize>().unwrap();
    //println!("size = {}", size);

    let mut data : Vec<u8> = vec![0; size+1];
    reader.read_exact(&mut data).unwrap();
    //println!("data = <{}>", String::from_utf8_lossy(&data));
    //println!("data = <{:?}>", data);

    let (_header, pixels) = data.split_at(9);
    let (pixels, _zero) = pixels.split_at(pixels.len()-1);

    assert_eq!(pixels.len(), (WINDOW_WIDTH * 3) as usize); // we received a line

    image[(relative_y * WINDOW_WIDTH * 3) as usize
          ..
          ((relative_y + 1) * WINDOW_WIDTH * 3) as usize].copy_from_slice(&pixels);
}

fn send_command(writer: &mut BufWriter<TcpStream>, command: &str) {
    let command_bytes = command.as_bytes();
    write!(writer, "{} ", command_bytes.len()).unwrap();
    writer.write_all(command_bytes).unwrap();
    writer.write(&vec![0]).unwrap();
    writer.flush().unwrap();
}

fn handle_connection(stream: TcpStream, id: u32, image: &mut [u8], y_min: u32, y_max: u32) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut writer = BufWriter::new(stream);

    let second = time::Duration::from_secs(1);

    println!(">>> [{}] Waiting LOGIN ...", id);
    receive_command(&mut reader);

    println!(">>> [{}] Sending INFO ...", id);
    let command_info = format!("INFO {} {}", WINDOW_WIDTH, WINDOW_HEIGHT);
    send_command(&mut writer, &command_info);

    thread::sleep(second);

    println!(">>> [{}] Sending SETSCENE ...", id);
    let command_info = format!("SETSCENE {}", XML_FILE);
    send_command(&mut writer, &command_info);

    thread::sleep(second);

    for y in y_min .. y_max+1 {

        println!(">>> [{}] Sending CALCULATE ({}/{}) ...", id, y-y_min+1, y_max-y_min+1);
        let command_info = format!("CALCULATE {} {} {} {}", 1, y, WINDOW_WIDTH, 1);
        send_command(&mut writer, &command_info);

        println!(">>> [{}] Waiting CALCULATING ...", id);
        receive_command(&mut reader);

        println!(">>> [{}] Waiting RESULT ...", id);
        receive_command_result(&mut reader, image, y-y_min);
    }
}
