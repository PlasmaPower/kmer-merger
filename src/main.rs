use std::cmp::Ordering;
use std::io;
use std::io::prelude::*;
use std::io::BufReader;
use std::fs::File;
use std::env;

#[macro_use]
extern crate log;
extern crate env_logger;

struct ParsedLine {
    kmer: Vec<u8>,
    present: bool,
}

struct InFile {
    reader: BufReader<File>,
    curr_line: ParsedLine,
}

struct KmerState {
    kmer: Vec<u8>,
    present: Vec<bool>,
}

fn parse_line(mut line: Vec<u8>, parsed_out: &mut ParsedLine) -> bool {
    let _ = line.pop().unwrap(); // newline
    let present_byte = line.pop().unwrap();
    let present = if present_byte == b'1' {
        true
    } else if present_byte == b'0' {
        false
    } else {
        line.push(present_byte); // Revert! Revert! haha
        error!("Encountered invalid line: \"{}\"",
               String::from_utf8_lossy(line.as_slice()));
        return false;
    };
    let separator = line.pop();
    if separator != Some(b' ') && separator != Some(b'\t') || line.len() == 0 {
        separator.map(|s| line.push(s));
        line.push(present_byte);
        error!("Encountered invalid line: \"{}\"",
               String::from_utf8_lossy(line.as_slice()));
        return false;
    }
    parsed_out.kmer = line;
    parsed_out.present = present;
    true
}

fn main() {
    env_logger::init().unwrap();

    let mut args = env::args();
    args.next(); // Program path

    let infilenames = args.collect::<Vec<_>>();
    if infilenames.is_empty() {
        panic!("No input files specified (as arguments)");
    }
    println!("{}", infilenames.join("\t"));

    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    let mut infiles = infilenames.iter()
        .map(|filename| {
            let reader = File::open(filename)
                .expect(format!("Could not open input file {}", filename).as_str());
            Some(InFile {
                reader: BufReader::new(reader),
                curr_line: ParsedLine {
                    kmer: Vec::new(),
                    present: false,
                },
            })
        })
        .collect::<Vec<_>>();

    info!("Merging files: {}", infilenames.join(", "));

    let file_count = infiles.len();
    let mut least_kmer = KmerState {
        kmer: Vec::new(),
        present: Vec::with_capacity(infiles.len()),
    };
    loop {
        for (file_index, infile) in infiles.iter_mut().enumerate() {
            let mut got_line = true;
            if let Some(ref mut infile) = *infile {
                if infile.curr_line.kmer.is_empty() {
                    loop {
                        // Note: we could move line_buf out of this loop,
                        // but it's only reused if the line is bad.
                        // We shouldn't need to worry about the performance of that.
                        let mut line_buf = Vec::new();
                        infile.reader.read_until(b'\n', &mut line_buf).unwrap();
                        match line_buf.len() {
                            0 => got_line = false,
                            1 => continue,
                            _ => {
                                if !parse_line(line_buf, &mut infile.curr_line) {
                                    continue;
                                }
                            }
                        }
                        break;
                    }
                }
                if got_line {
                    let ref line = infile.curr_line;
                    if least_kmer.present.is_empty() ||
                       match line.kmer.cmp(&mut least_kmer.kmer) {
                        Ordering::Less => true,
                        Ordering::Equal => {
                            least_kmer.present[file_index] = true;
                            false
                        }
                        Ordering::Greater => false,
                    } {
                        least_kmer.present.clear();
                        for i in 0..file_count {
                            if i == file_index {
                                least_kmer.present.push(line.present);
                            } else {
                                least_kmer.present.push(false);
                            }
                        }
                        least_kmer.kmer = line.kmer.clone();
                    }
                }
            }
            if !got_line {
                *infile = None;
            }
        }
        if least_kmer.present.is_empty() {
            break;
        }
        for (i, file_present) in least_kmer.present.iter().enumerate() {
            if *file_present {
                if let Some(ref mut file) = infiles[i] {
                    file.curr_line.kmer.clear();
                }
            }
        }
        stdout.write(least_kmer.kmer.as_slice()).unwrap();
        stdout.write(b"\t").unwrap();
        let mut present_fmt = Vec::with_capacity(least_kmer.present.len() * 2);
        for p in least_kmer.present.iter().cloned() {
            present_fmt.push(if p { b'1' } else { b'0' });
            present_fmt.push(b'\t');
        }
        stdout.write(present_fmt.as_slice()).unwrap();
        stdout.write(b"\n").unwrap();
        least_kmer.kmer.clear();
        least_kmer.present.clear();
    }

    info!("Done");
}
