use std::collections::BinaryHeap;
use std::collections::HashSet;
use std::io;
use std::io::Write;
use std::env;

#[macro_use]
extern crate log;
extern crate env_logger;

extern crate memmap;
extern crate memchr;

use memmap::Mmap;

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
    let mut seen_infiles = HashSet::new();
    for filename in infilenames.iter().cloned() {
        if seen_infiles.contains(&filename) {
            error!("Duplicate input file: {}", filename);
            return;
        } else {
            seen_infiles.insert(filename);
        }
    }
    let file_count = infilenames.len();
    println!("Kmer\t{}", infilenames.join("\t"));

    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    let mut infiles = infilenames.iter().enumerate()
        .map(|(index, filename)| {
            let mmap = Mmap::open_path(filename, memmap::Protection::Read).expect(format!("Could not open input file {}", filename).as_str());
            let position_info = infile::PositionInfo {
                index: index,
                out_of: file_count,
            };
            unsafe { InFile::new(mmap, position_info) }
        })
    .collect::<BinaryHeap<_>>();

    info!("Merging files: {}", infilenames.join(", "));

    let mut next_kmer = infiles.peek_mut().and_then(|mut file| unsafe { file.advance() });
    while let Some(mut curr_kmer) = next_kmer.take() {
        while let Some(mut infile) = infiles.pop() {
            let index = infile.position.index;
            if let Some(read_kmer) = unsafe { infile.advance() } {
                infiles.push(infile);
                // We check this in reverse because the elements are close together.
                // That means that their starts will very likely be equal, but their
                // endings will very likely not be equal.
                // Note: this computation has actually been done before when
                // the element is inserted into the BinaryHeap. This might be
                // a small future optimization.
                if unsafe { read_kmer.kmer.as_slice().iter().rev().eq(curr_kmer.kmer.as_slice().iter().rev()) } {
                    curr_kmer.present[index] = true;
                } else {
                    next_kmer = Some(read_kmer);
                    break;
                }
            }
        }
        stdout.write(unsafe { curr_kmer.kmer.as_slice() }).unwrap();
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
