use std::collections::BinaryHeap;
use std::io;
use std::io::Write;
use std::io::BufReader;
use std::fs::File;
use std::env;

#[macro_use]
extern crate log;
extern crate env_logger;

mod infile;
use infile::InFile;

fn main() {
    env_logger::init().unwrap();

    let mut args = env::args();
    args.next(); // Program path

    let infilenames = args.collect::<Vec<_>>();
    if infilenames.is_empty() {
        error!("No input files specified (as arguments)");
        return;
    }
    let file_count = infilenames.len();
    println!("kmer\t{}", infilenames.join("\t"));

    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    let mut infiles = infilenames.iter().enumerate()
        .map(|(index, filename)| {
            let reader = File::open(filename)
                .expect(format!("Could not open input file {}", filename).as_str());
            InFile::new(BufReader::new(reader), infile::PositionInfo {
                index: index,
                out_of: file_count,
            })
        })
    .collect::<BinaryHeap<_>>();

    info!("Merging files: {}", infilenames.join(", "));

    let mut next_kmer = infiles.peek_mut().and_then(|mut file| file.advance());
    while let Some(mut curr_kmer) = next_kmer.take() {
        while let Some(mut infile) = infiles.pop() {
            let index = infile.position.index;
            if let Some(read_kmer) = infile.advance() {
                infiles.push(infile);
                // We check this in reverse because the elements are close together.
                // That means that their starts will very likely be equal, but their
                // endings will very likely not be equal.
                // Note: this computation has actually been done before when
                // the element is inserted into the BinaryHeap. This might be
                // a small future optimization.
                if read_kmer.kmer.iter().rev().eq(curr_kmer.kmer.iter().rev()) {
                    curr_kmer.present[index] = true;
                } else {
                    next_kmer = Some(read_kmer);
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
