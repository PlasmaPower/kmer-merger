use std::cmp::Ordering;
use std::io::BufRead;
use std::io::BufReader;
use std::fs::File;

pub struct PositionInfo {
    pub index: usize,
    pub out_of: usize,
}

pub struct KmerState {
    pub kmer: Vec<u8>,
    pub present: Vec<bool>,
}

struct ParsedLine {
    kmer: Vec<u8>,
    present: bool,
}

impl ParsedLine {
    fn into_kmer_state(self, position: &PositionInfo) -> KmerState {
        let mut present_array = Vec::with_capacity(position.out_of);
        for i in 0..position.out_of {
            present_array.push(if i == position.index {
                self.present
            } else {
                false
            });
        }
        KmerState {
            kmer: self.kmer,
            present: present_array,
        }
    }
}

fn parse_line(mut line: Vec<u8>) -> Option<ParsedLine> {
    // Panics if line.is_empty()
    let newline = line.pop().unwrap();
    if newline != b'\n' {
        line.push(newline); // Oops!
    }
    let present_byte = line.pop();
    let present = if present_byte == Some(b'1') {
        true
    } else if present_byte == Some(b'0') {
        false
    } else {
        present_byte.map(|c| line.push(c));
        error!("Encountered invalid line: \"{}\"",
               String::from_utf8_lossy(line.as_slice()));
        return None;
    };
    let separator = line.pop();
    if separator != Some(b' ') && separator != Some(b'\t') || line.len() == 0 {
        separator.map(|c| line.push(c));
        present_byte.map(|c| line.push(c));
        error!("Encountered invalid line: \"{}\"",
               String::from_utf8_lossy(line.as_slice()));
        return None;
    }
    Some(ParsedLine {
        kmer: line,
        present: present,
    })
}

pub struct InFile {
    pub position: PositionInfo,
    reader: BufReader<File>,
    curr_line: Option<ParsedLine>,
    inverted: bool,
}

impl InFile {
    pub fn new(reader: BufReader<File>, inverted: bool, position: PositionInfo) -> InFile {
        let mut infile = InFile {
            reader: reader,
            curr_line: None,
            position: position,
            inverted: inverted,
        };
        infile.advance();
        infile
    }

    /// Advances the file, filline curr_line and returning the old one
    /// Will only return None if the file is finished
    /// In case of an error, the function will simply panic
    pub fn advance(&mut self) -> Option<KmerState> {
        let prev_line = self.curr_line.take();
        loop {
            let mut line = Vec::new();
            self.reader.read_until(b'\n', &mut line).unwrap();
            if line.is_empty() {
                // Not even a newline, no data left in file.
                // Leaves self.prev_line blank.
                break;
            } else if line != b"\n" {
                // Don't break if parse_line fails
                if let Some(mut parsed_line) = parse_line(line) {
                    if self.inverted {
                        // Probably optimized down to an XOR,
                        // but this is easier to read.
                        parsed_line.present = !parsed_line.present;
                    }
                    self.curr_line = Some(parsed_line);
                    break;
                }
            }
        }
        return prev_line.map(|line| line.into_kmer_state(&self.position));
    }
}

impl Ord for InFile {
    /// The greatest element is the one with the least curr_line
    /// Or, in other words, the one that needs to be processed next
    /// Primary for use with BinaryHeap
    fn cmp(&self, other: &Self) -> Ordering {
        match self.curr_line {
            None => {
                match other.curr_line {
                    None => Ordering::Equal,
                    Some(_) => Ordering::Less,
                }
            }
            Some(ref self_line) => {
                match other.curr_line {
                    None => Ordering::Greater,
                    // Reverse comparison here is intentional
                    Some(ref other_line) => other_line.kmer.cmp(&self_line.kmer),
                }
            }
        }
    }
}

impl PartialOrd for InFile {
    fn partial_cmp(&self, other: &InFile) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for InFile {
    fn eq(&self, other: &InFile) -> bool {
        match self.curr_line {
            None => {
                match other.curr_line {
                    None => true,
                    Some(_) => false,
                }
            }
            Some(ref self_line) => {
                match other.curr_line {
                    None => false,
                    Some(ref other_line) => self_line.kmer.eq(&other_line.kmer),
                }
            }
        }
    }
}

impl Eq for InFile {}
