#[macro_use]
extern crate clap;
extern crate crossbeam_utils;
extern crate png;

use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::io::BufWriter;
use std::net::TcpListener;
use std::net::TcpStream;
use std::path::Path;
use std::str;
use std::time::Duration;

use clap::Parser;

#[derive(Parser)]
#[clap(version = crate_version!())]
struct Opts {
    /// Path to XML scene file
    #[clap(short = 's')]
    scene_file: String,
    /// Image width (must be divisible by 64)
    #[clap(short = 'w', name = "WIDTH")]
    image_width: usize,
    /// Image height
    #[clap(short = 'y', name = "HEIGHT")]
    image_height: usize,
    /// Expected number of clients
    #[clap(short = 'c')]
    clients_count: usize,
    /// Client computation timeout (in seconds)
    #[clap(short = 't', name = "TIMEOUT", default_value = "10")]
    client_computation_timeout: u64,
    /// Add a level of verbosity (can be used multiple times)
    #[clap(short = 'v', parse(from_occurrences))]
    verbosity_level: u8,
}

// while waiting for int_roundings
// https://github.com/rust-lang/rfcs/issues/2844
// https://github.com/rust-lang/rust/issues/88581
fn div_ceil(lhs: usize, rhs: usize) -> usize {
    let d = lhs / rhs;
    let r = lhs % rhs;
    if r > 0 {
        d + 1
    } else {
        d
    }
}

fn main() {
    let opts: Opts = Opts::parse();
    assert!(opts.image_width % 64 == 0);

    let image_data_size: usize = opts.image_width * opts.image_height * 3;

    let listener = TcpListener::bind("127.0.0.1:1234").unwrap();

    let path = Path::new("image.png");
    let file = File::create(path).unwrap();
    let image_writer = &mut BufWriter::new(file);
    let mut encoder = png::Encoder::new(
        image_writer,
        opts.image_width as u32,
        opts.image_height as u32,
    );
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer_png = encoder.write_header().unwrap();
    let mut image_data = vec![127u8; image_data_size];

    if opts.verbosity_level >= 1 {
        println!("Listening...");
    }

    let chunk_line_count = div_ceil(opts.image_height, opts.clients_count);
    let chunk_size = chunk_line_count * opts.image_width * 3;

    crossbeam_utils::thread::scope(|sc| {
        for (i, image_chunk) in image_data.chunks_mut(chunk_size).enumerate() {
            let id: u32 = (i + 1) as u32;

            let (stream, _addr) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(opts.client_computation_timeout)))
                .unwrap();
            if opts.verbosity_level >= 1 {
                println!(">>> [{}] Connection established", id);
            }

            let scene_file = opts.scene_file.clone();
            let image_width = opts.image_width;
            let image_height = opts.image_height;
            let verbosity_level = opts.verbosity_level;

            sc.builder()
                .name(format!("client_{}", id))
                .spawn(move |_| {
                    handle_connection(
                        stream,
                        id,
                        scene_file,
                        image_width,
                        image_height,
                        image_chunk,
                        i * chunk_line_count,
                        usize::min((i + 1) * chunk_line_count - 1, image_height - 1),
                        verbosity_level,
                    );
                    if verbosity_level >= 1 {
                        println!(">>> [{}] Finished", id);
                    }
                })
                .unwrap();
        }
    })
    .expect("Failed to close client threads scope");

    if opts.verbosity_level >= 1 {
        println!(">>> Saving image ...");
    }
    writer_png.write_image_data(&image_data).unwrap();
}

fn receive_command(reader: &mut BufReader<TcpStream>, verbosity_level: u8) {
    let mut size_bytes: Vec<u8> = vec![];
    reader.read_until(b' ', &mut size_bytes).unwrap();
    size_bytes.truncate(size_bytes.len() - 1);
    let size: usize = str::from_utf8(&size_bytes)
        .unwrap()
        .parse::<usize>()
        .unwrap();
    if verbosity_level >= 3 {
        println!("size = {}", size);
    }

    let mut data: Vec<u8> = vec![0; size + 1];
    reader.read_exact(&mut data).unwrap();
    if verbosity_level >= 3 {
        println!("data = <{}>", String::from_utf8_lossy(&data));
    }
}

fn receive_command_result(
    reader: &mut BufReader<TcpStream>,
    image: &mut [u8],
    image_width: usize,
    relative_y: usize,
    verbosity_level: u8,
) {
    let mut size_bytes: Vec<u8> = vec![];
    reader.read_until(b' ', &mut size_bytes).unwrap();
    size_bytes.truncate(size_bytes.len() - 1);
    let size: usize = str::from_utf8(&size_bytes)
        .unwrap()
        .parse::<usize>()
        .unwrap();
    if verbosity_level >= 3 {
        println!("size = {}", size);
    }

    let mut data: Vec<u8> = vec![0; size + 1];
    reader.read_exact(&mut data).unwrap();
    if verbosity_level >= 4 {
        println!("data = <{}>", String::from_utf8_lossy(&data));
        println!("data = <{:?}>", data);
    }

    let (_header, pixels) = data.split_at(9);
    let (pixels, _zero) = pixels.split_at(pixels.len() - 1);

    assert_eq!(pixels.len(), image_width * 3); // we received a complete line

    image[relative_y * image_width * 3..(relative_y + 1) * image_width * 3].copy_from_slice(pixels);
}

fn send_command(writer: &mut BufWriter<TcpStream>, command: &str) {
    let command_bytes = command.as_bytes();
    write!(writer, "{} ", command_bytes.len()).unwrap();
    writer.write_all(command_bytes).unwrap();
    writer.write_all(&[0]).unwrap();
    writer.flush().unwrap();
}

fn handle_connection(
    stream: TcpStream,
    id: u32,
    scene_file: String,
    image_width: usize,
    image_height: usize,
    image: &mut [u8],
    y_min: usize,
    y_max: usize,
    verbosity_level: u8,
) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut writer = BufWriter::new(stream);

    if verbosity_level >= 2 {
        println!(">>> [{}] Waiting LOGIN ...", id);
    }
    receive_command(&mut reader, verbosity_level); // LOGIN

    if verbosity_level >= 2 {
        println!(">>> [{}] Sending INFO ...", id);
    }
    let command_info = format!("INFO {} {}", image_width, image_height);
    send_command(&mut writer, &command_info);

    receive_command(&mut reader, verbosity_level); // INFODONE

    if verbosity_level >= 2 {
        println!(">>> [{}] Sending SETSCENE ...", id);
    }
    let command_info = format!("SETSCENE {}", scene_file);
    send_command(&mut writer, &command_info);

    receive_command(&mut reader, verbosity_level); // SETSCENEDONE

    for y in y_min..=y_max {
        if verbosity_level >= 2 {
            println!(
                ">>> [{}] Sending CALCULATE ({}/{}) ...",
                id,
                y - y_min + 1,
                y_max - y_min + 1
            );
        }
        let command_info = format!("CALCULATE {} {} {} {}", 1, y, image_width, 1);
        send_command(&mut writer, &command_info);

        if verbosity_level >= 2 {
            println!(">>> [{}] Waiting RESULT ...", id);
        }
        receive_command_result(&mut reader, image, image_width, y - y_min, verbosity_level);
    }
}
