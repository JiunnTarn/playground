use crossterm::{cursor, execute, style, terminal};
use nfd::Response;
use num_format::{Locale, ToFormattedString};
use std::collections::HashMap;
use std::fs::File;
use std::io::{stdout, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

const MAX_DICT_SIZE: u16 = 65535; // 2^16
const BRAILLE: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

fn compress(input: &str, output: &str) {
    execute!(stdout(), cursor::Hide).unwrap();
    std::thread::spawn(move || loop {
        for braille in &BRAILLE {
            print!("\r{} Compressing...", braille);
            stdout().flush().unwrap();
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    });

    let origin_size = std::fs::metadata(input).unwrap().len();

    let mut dict_size: u16 = 256;
    let mut dictionary: HashMap<Vec<u8>, u16> = HashMap::new();
    for i in 0u8..=255u8 {
        dictionary.insert(vec![i], i as u16);
    }

    let mut input_file = File::open(input).unwrap();
    let mut input_data = Vec::new();
    input_file.read_to_end(&mut input_data).unwrap();

    let mut output_file = File::create(output).unwrap();
    let mut prev = vec![];
    let mut result = vec![];

    for &cur in input_data.iter() {
        let mut p_and_c = prev.clone();
        p_and_c.push(cur);
        if dictionary.contains_key(&p_and_c) {
            prev = p_and_c;
        } else {
            let code = dictionary.get(&prev).unwrap();
            result.write_all(&code.to_be_bytes()).unwrap();
            if dict_size < MAX_DICT_SIZE {
                dictionary.insert(p_and_c, dict_size);
                dict_size += 1;
            }
            prev = vec![cur];
        }
    }
    if !prev.is_empty() {
        let code = dictionary.get(&prev).unwrap();
        result.write_all(&code.to_be_bytes()).unwrap();
    }
    output_file.write_all(&result).unwrap();

    let compressed_size = std::fs::metadata(output).unwrap().len();
    let compressio_ratio = compressed_size as f64 / origin_size as f64;

    execute!(
        stdout(),
        terminal::Clear(terminal::ClearType::CurrentLine),
        cursor::MoveToColumn(0),
        style::SetForegroundColor(style::Color::Green)
    )
    .unwrap();
    println!(
        "✓ Done!    {} bytes -> {} bytes ({:.2}% reduction)",
        origin_size.to_formatted_string(&Locale::en),
        compressed_size.to_formatted_string(&Locale::en),
        (1f64 - compressio_ratio) * 100f64
    );
    execute!(stdout(), style::ResetColor, cursor::Show).unwrap();
}

fn decompress(input: &str, output: &str) {
    execute!(stdout(), cursor::Hide).unwrap();
    std::thread::spawn(move || loop {
        for braille in &BRAILLE {
            print!("\r{} Decompressing...", braille);
            stdout().flush().unwrap();
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    });

    let mut dict_size: u16 = 256;
    let mut dictionary: HashMap<u16, Vec<u8>> = HashMap::new();
    for i in 0u8..=255u8 {
        dictionary.insert(i as u16, vec![i]);
    }

    let mut input_file = File::open(input).unwrap();
    let mut input_data = Vec::new();
    input_file.read_to_end(&mut input_data).unwrap();

    let mut output_file = File::create(output).unwrap();
    let mut result = vec![];

    let pre: u16 = u16::from_be_bytes(
        input_data[0..0 + std::mem::size_of::<u16>()]
            .try_into()
            .unwrap(),
    );
    let pre_entry: Vec<u8> = dictionary.get(&pre).unwrap().to_vec();
    result.extend_from_slice(&pre_entry);

    let mut cur: u16;
    let mut prev: Vec<u8> = pre_entry;
    let mut entry: Vec<u8>;

    for index in (std::mem::size_of::<u16>()..input_data.len()).step_by(std::mem::size_of::<u16>())
    {
        cur = u16::from_be_bytes(
            input_data[index..index + std::mem::size_of::<u16>()]
                .try_into()
                .unwrap(),
        );
        if dictionary.contains_key(&cur) {
            entry = dictionary.get(&cur).unwrap().to_vec();
        } else if cur == dict_size {
            entry = prev.clone();
            entry.push(prev[0]);
        } else {
            panic!("Bad compressed! cur: {}", cur);
        }

        result.extend_from_slice(&entry);
        let mut p_and_c = prev.clone();
        p_and_c.push(entry[0]);
        if dict_size < MAX_DICT_SIZE {
            dictionary.insert(dict_size, p_and_c);
            dict_size += 1;
        }
        prev = entry.clone();
    }
    output_file.write_all(&result).unwrap();

    execute!(
        stdout(),
        terminal::Clear(terminal::ClearType::CurrentLine),
        cursor::MoveToColumn(0),
        style::SetForegroundColor(style::Color::Green)
    )
    .unwrap();
    println!("✓ Done!");
    execute!(stdout(), style::ResetColor, cursor::Show).unwrap();
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 1 {
        let result = nfd::open_file_multiple_dialog(None, None).unwrap_or_else(|e| {
            panic!("Error: {:?}", e);
        });

        match result {
            Response::Okay(file_path) => {
                let input = file_path.clone();
                let path = PathBuf::from(input.clone());
                let parent_dir = path.parent().unwrap();

                if input[input.len() - 4..] == String::from(".lzw") {
                    let output = &input[..input.len() - 4].to_string();
                    decompress(&input, output);

                    Command::new("explorer")
                        .arg(parent_dir)
                        .status()
                        .expect("无法打开文件夹");
                    return;
                }
                let output = file_path.clone() + ".lzw";
                compress(&input, &output);

                Command::new("explorer")
                    .arg(parent_dir)
                    .status()
                    .expect("无法打开文件夹");
                return;
            }
            Response::OkayMultiple(file_paths) => {
                let mut input;
                let mut path;
                let mut parent_dir = Path::new(".");

                for file_path in file_paths.iter() {
                    input = file_path.clone();
                    path = PathBuf::from(input.clone());
                    parent_dir = path.parent().unwrap();

                    if input[input.len() - 4..] == String::from(".lzw") {
                        let output = &input[..input.len() - 4].to_string();
                        decompress(&input, output);
                        
                        continue;
                    }
                    let output = file_path.clone() + ".lzw";
                    compress(&input, &output);
                }

                Command::new("explorer")
                .arg(parent_dir)
                .status()
                .expect("无法打开文件夹");
                return;
            }
            Response::Cancel => {
                return;
            }
        }
    } else if args.len() < 4 {
        println!("Usage: lzw <compress/decompress> <input_file> <output_file>");
        return;
    } else {
        let input = args[2].clone();
        let output = args[3].clone();

        match &args[1][..] {
            "compress" => compress(&input, &output),
            "decompress" => decompress(&input, &output),
            _ => println!("Invalid command"),
        }
    }
}
