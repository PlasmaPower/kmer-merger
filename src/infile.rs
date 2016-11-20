use std::cmp::Ordering;

use memchr::memchr;
use memmap::Mmap;
use memmap::MmapView;

const NULL_BYTE: &'static u8 = &b'\0';

pub struct PositionInfo {
    pub index: usize,
    pub out_of: usize,
}

pub struct KmerState {
    pub kmer: MmapView,
    pub present: Vec<bool>,
}

struct ParsedLine {
    kmer: MmapView,
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

unsafe fn parse_line(line_view: MmapView) -> Option<ParsedLine> {
    let (split_at, present) = {
        let line = line_view.as_slice();
        let mut bound = line.len();
        let newline = line.last().unwrap();
        if newline == &b'\n' {
            bound -= 1;
        }
        bound -= 1;
        let present_byte = line.get(bound).unwrap_or(&NULL_BYTE);
        let present = if present_byte == &b'1' {
            true
        } else if present_byte == &b'0' {
            false
        } else {
            error!("Encountered invalid line: \"{}\"",
                   String::from_utf8_lossy(line));
            return None;
        };
        bound -= 1;
        let separator = line.get(bound).unwrap_or(&NULL_BYTE);
        if separator != &b' ' && separator != &b'\t' || line.len() == 0 {
            error!("Encountered invalid line: \"{}\"",
                   String::from_utf8_lossy(line));
            return None;
        }
        (bound, present)
    };
    let line_split = line_view.split_at(split_at).unwrap();
    Some(ParsedLine {
        kmer: line_split.0,
        present: present,
    })
}

pub struct InFile {
    pub position: PositionInfo,
    file: Option<MmapView>,
    curr_line: Option<ParsedLine>,
}

impl InFile {
    pub unsafe fn new(mmap: Mmap, position: PositionInfo) -> InFile {
        let mut infile = InFile {
            file: Some(mmap.into_view()),
            curr_line: None,
            position: position,
        };
        infile.advance();
        infile
    }

    /// Advances the file, filline curr_line and returning the old one
    /// Will only return None if the file is finished
    /// In case of an error, the function will simply panic
    pub unsafe fn advance(&mut self) -> Option<KmerState> {
        let prev_line = self.curr_line.take();
        loop {
            let file = match self.file.take() {
                Some(f) => f,
                None => break,
            };
            let newline_index = memchr(b'\n', file.as_slice());
            let line = if let Some(newline_index) = newline_index {
                let file_split = file.split_at(newline_index + 1).unwrap();
                self.file = Some(file_split.1);
                file_split.0
            } else {
                file
            };
            if line.len() == 0 {
                break;
            }
            // Don't break if parse_line fails
            if let Some(parsed_line) = parse_line(line) {
                self.curr_line = Some(parsed_line);
                break;
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
                    Some(ref other_line) => unsafe { other_line.kmer.as_slice().cmp(&self_line.kmer.as_slice()) },
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
                    Some(ref other_line) => unsafe { self_line.kmer.as_slice().eq(other_line.kmer.as_slice()) },
                }
            }
        }
    }
}

impl Eq for InFile {}
