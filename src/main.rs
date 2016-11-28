use std::collections::BinaryHeap;
use std::io;
use std::io::Write;
use std::io::BufReader;
use std::fs::File;

#[macro_use]
extern crate log;
extern crate env_logger;

extern crate clap;
use clap::App;
use clap::Arg;

mod infile;
use infile::InFile;

fn main() {
    env_logger::init().unwrap();

    let args = App::new("kmer-merger")
        .arg(Arg::with_name("inputs")
             .value_name("INPUTS")
             .help("The list of input files")
             .multiple(true)
             .required_unless("inverted-inputs"))
        .arg(Arg::with_name("inverted-inputs")
             .short("i")
             .long("inverted")
             .value_name("INVERTED-INPUTS")
             .help("The list of inverted input files (independent of INPUTS, usually placed afterwords)")
             .multiple(true))
        .get_matches();

    let infilenames = args.values_of("inputs").unwrap().collect::<Vec<_>>();
    let invertedfilenames = args.values_of("inverted-inputs").unwrap().collect::<Vec<_>>();
    let file_count = infilenames.len() + invertedfilenames.len();
    println!("kmer\t{}\t{}", infilenames.join("\t"), invertedfilenames.join("\t"));

    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    let mut infiles = infilenames.iter().map(|file| (file, false))
        .chain(invertedfilenames.iter().map(|file| (file, true)))
        .enumerate()
        .map(|(index, (filename, inverted))| {
            let reader = File::open(filename)
                .expect(format!("Could not open input file {}", filename).as_str());
            InFile::new(BufReader::new(reader), inverted, infile::PositionInfo {
                index: index,
                out_of: file_count,
            })
        })
    .collect::<BinaryHeap<_>>();

    info!("Merging files: {}, inverted: {}", infilenames.join(", "), invertedfilenames.join(", "));

    let mut next_kmer = infiles.peek_mut().and_then(|mut file| {
        file.advance().map(|line| line.into_kmer_state(&file.position))
    });
    while let Some(mut curr_kmer) = next_kmer.take() {
        while let Some(mut infile) = infiles.pop() {
            let index = infile.position.index;
            if let Some(read_kmer) = infile.advance() {
                // We check this in reverse because the elements are close together.
                // That means that their starts will very likely be equal, but their
                // endings will very likely not be equal.
                // Note: this computation has actually been done before when
                // the element is inserted into the BinaryHeap. This might be
                // a small future optimization.
                let next_equal = read_kmer.kmer.iter().rev().eq(curr_kmer.kmer.iter().rev());
                if next_equal {
                    curr_kmer.present[index] = read_kmer.present;
                } else {
                    next_kmer = Some(read_kmer.into_kmer_state(&infile.position));
                }
                infiles.push(infile);
                if !next_equal {
                    break;
                }
            }
        }
        stdout.write(curr_kmer.kmer.as_slice()).unwrap();
        let mut present_fmt = Vec::with_capacity(curr_kmer.present.len() * 2 + 1);
        for p in curr_kmer.present.iter() {
            present_fmt.push(b'\t');
            present_fmt.push(if *p { b'1' } else { b'0' });
        }
        present_fmt.push(b'\n');
        stdout.write(present_fmt.as_slice()).unwrap();
    }

    info!("Done");
}
