#[macro_use]
extern crate clap;
extern crate crossbeam_utils;
extern crate png;

use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::io::BufWriter;
use std::net::SocketAddr;
use std::net::TcpListener;
use std::net::TcpStream;
use std::num::NonZeroU32;
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

struct Client {
    id: NonZeroU32,
    address: SocketAddr,
    name: String,
    reader: BufReader<TcpStream>,
    writer: BufWriter<TcpStream>,
}

enum CameraMovement {
    TranslateForward,
    // TranslateBackwards,
    // TranslateRight,
    // TranslateLeft,
    // TranslateUp,
    // TranslateDown,
    // RollLeft,
    // RollRight,
    // PitchUp,
    // PitchDown,
    // YawLeft,
    // YawRight,
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

    let image_data_size = opts.image_width * opts.image_height * 3;
    let mut image_data = vec![127u8; image_data_size];

    let mut clients = Vec::with_capacity(opts.clients_count);

    get_clients(
        &mut clients,
        opts.clients_count,
        opts.client_computation_timeout,
        opts.verbosity_level,
    );

    initialize_all(
        &mut clients,
        &opts.scene_file,
        opts.image_width,
        opts.image_height,
        opts.verbosity_level,
    );

    for i in 1..=5 {
        render_all(
            &mut clients,
            &mut image_data,
            opts.image_width,
            opts.image_height,
            opts.verbosity_level,
        );

        let path = format!("image{}.png", i);

        save_image(
            &path,
            &image_data,
            opts.image_width,
            opts.image_height,
            opts.verbosity_level,
        );

        move_camera_all(
            &mut clients,
            CameraMovement::TranslateForward,
            opts.verbosity_level,
        );
    }
}

fn receive_command(
    reader: &mut BufReader<TcpStream>,
    expected_command: &str,
    verbosity_level: u8,
) -> Option<String> {
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
    let (message, _zero) = data.split_at(data.len() - 1);
    let message = str::from_utf8(message).unwrap();
    let (command, arguments) = if message.contains(' ') {
        let (left, right) = message.split_once(' ').unwrap();
        (left, Some(right))
    } else {
        (message, None)
    };

    if verbosity_level >= 3 {
        println!("command = <{}>", command);
        if let Some(args) = arguments {
            println!("arguments = <{}>", args)
        }
    }

    assert_eq!(command, expected_command);

    arguments.map(String::from)
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

fn get_clients(
    clients: &mut Vec<Client>,
    clients_count: usize,
    client_computation_timeout: u64,
    verbosity_level: u8,
) {
    let listener = TcpListener::bind("127.0.0.1:1234").unwrap();

    if verbosity_level >= 1 {
        println!(">>> Listening...");
    }

    for i in 1..=clients_count {
        let (stream, address) = listener.accept().unwrap();

        stream
            .set_read_timeout(Some(Duration::from_secs(client_computation_timeout)))
            .unwrap();

        let id = NonZeroU32::new(i as u32).unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let writer = BufWriter::new(stream);

        if verbosity_level >= 2 {
            println!(">>> [{}] Waiting LOGIN ...", id);
        }
        let name = receive_command(&mut reader, "LOGIN", verbosity_level).unwrap();

        let client = Client {
            id,
            address,
            name,
            reader,
            writer,
        };
        if verbosity_level >= 1 {
            println!(
                ">>> [{}] Connection established from {} ({})",
                client.id, client.address, client.name
            );
        }
        clients.push(client)
    }

    if verbosity_level >= 1 {
        println!(">>> All clients are connected");
    }
}

fn initialize(
    client: &mut Client,
    scene_file: &str,
    image_width: usize,
    image_height: usize,
    verbosity_level: u8,
) {
    if verbosity_level >= 2 {
        println!(">>> [{}] Sending INFO ...", client.id);
    }
    let command_info = format!("INFO {} {}", image_width, image_height);
    send_command(&mut client.writer, &command_info);

    receive_command(&mut client.reader, "INFODONE", verbosity_level);

    if verbosity_level >= 2 {
        println!(">>> [{}] Sending SETSCENE ...", client.id);
    }
    let command_info = format!("SETSCENE {}", scene_file);
    send_command(&mut client.writer, &command_info);

    receive_command(&mut client.reader, "SETSCENEDONE", verbosity_level);
}

fn initialize_all(
    clients: &mut Vec<Client>,
    scene_file: &str,
    image_width: usize,
    image_height: usize,
    verbosity_level: u8,
) {
    crossbeam_utils::thread::scope(|sc| {
        for client in clients.iter_mut() {
            sc.builder()
                .name(format!("init_{}", client.id))
                .spawn(move |_| {
                    initialize(
                        client,
                        scene_file,
                        image_width,
                        image_height,
                        verbosity_level,
                    );
                    if verbosity_level >= 1 {
                        println!(">>> [{}] Initialized", client.id);
                    }
                })
                .unwrap();
        }
    })
    .expect("Failed to close client threads scope");

    if verbosity_level >= 1 {
        println!(">>> All clients are initialized");
    }
}

fn render(
    client: &mut Client,
    image_width: usize,
    image: &mut [u8],
    y_min: usize,
    y_max: usize,
    verbosity_level: u8,
) {
    for y in y_min..=y_max {
        if verbosity_level >= 2 {
            println!(
                ">>> [{}] Sending CALCULATE ({}/{}) ...",
                client.id,
                y - y_min + 1,
                y_max - y_min + 1
            );
        }
        let command_info = format!("CALCULATE {} {} {} {}", 1, y, image_width, 1);
        send_command(&mut client.writer, &command_info);

        if verbosity_level >= 2 {
            println!(">>> [{}] Waiting RESULT ...", client.id);
        }
        receive_command_result(
            &mut client.reader,
            image,
            image_width,
            y - y_min,
            verbosity_level,
        );
    }
}

fn render_all(
    clients: &mut Vec<Client>,
    image_data: &mut Vec<u8>,
    image_width: usize,
    image_height: usize,
    verbosity_level: u8,
) {
    let chunk_line_count = div_ceil(image_height, clients.len());
    let chunk_size = chunk_line_count * image_width * 3;

    crossbeam_utils::thread::scope(|sc| {
        for (i, (client, image_chunk)) in clients
            .iter_mut()
            .zip(image_data.chunks_mut(chunk_size))
            .enumerate()
        {
            sc.builder()
                .name(format!("render_{}", client.id))
                .spawn(move |_| {
                    render(
                        client,
                        image_width,
                        image_chunk,
                        i * chunk_line_count,
                        usize::min((i + 1) * chunk_line_count - 1, image_height - 1),
                        verbosity_level,
                    );
                    if verbosity_level >= 1 {
                        println!(">>> [{}] Finished render rendering", client.id);
                    }
                })
                .unwrap();
        }
    })
    .expect("Failed to close client threads scope");

    if verbosity_level >= 1 {
        println!(">>> All clients finished rendering");
    }
}

fn save_image(
    path: &str,
    image_data: &[u8],
    image_width: usize,
    image_height: usize,
    verbosity_level: u8,
) {
    let path = Path::new(path);
    let file = File::create(path).unwrap();
    let image_writer = &mut BufWriter::new(file);
    let mut encoder = png::Encoder::new(image_writer, image_width as u32, image_height as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer_png = encoder.write_header().unwrap();
    writer_png.write_image_data(image_data).unwrap();

    if verbosity_level >= 1 {
        println!(">>> Image {} saved ...", path.display());
    }
}

fn move_camera_all(clients: &mut Vec<Client>, movement: CameraMovement, verbosity_level: u8) {
    let command_info = format!(
        "CAM {}",
        match movement {
            CameraMovement::TranslateForward => "tF",
            // CameraMovement::TranslateBackwards => "tB",
            // CameraMovement::TranslateRight => "tR",
            // CameraMovement::TranslateLeft => "tL",
            // CameraMovement::TranslateUp => "tU",
            // CameraMovement::TranslateDown => "tD",
            // CameraMovement::RollLeft => "rL",
            // CameraMovement::RollRight => "rR",
            // CameraMovement::PitchUp => "pU",
            // CameraMovement::PitchDown => "pD",
            // CameraMovement::YawLeft => "yL",
            // CameraMovement::YawRight => "yR",
        }
    );

    for client in clients.iter_mut() {
        if verbosity_level >= 2 {
            println!(">>> [{}] Sending CAM ...", client.id);
        }

        send_command(&mut client.writer, &command_info);

        receive_command(&mut client.reader, "CAMDONE", verbosity_level);
    }

    if verbosity_level >= 1 {
        println!(">>> All clients moved the camera");
    }
}
