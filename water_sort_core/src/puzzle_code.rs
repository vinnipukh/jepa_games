//! Puzzle codes: a compact, copy-pasteable encoding of params + state (D7).
//!
//! Layout, before base32: the bytes `[version, n_colors, capacity, n_empty]`, then every cell,
//! tube by tube, bottom to top, `capacity` cells per tube, as `bit_width(n_colors)` bits each
//! (0 = empty, `c + 1` = color `c`), most significant bit first, zero-padded to a whole byte.
//! The bytes are written in Crockford base32 (most significant bits first, no padding).

use crate::params::Params;
use crate::state::{EMPTY, State, StateError};

/// Format version, the first byte of every code.
pub const VERSION: u8 = 1;

const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Why a puzzle code cannot be decoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PuzzleCodeError {
    #[error("invalid character {0:?}")]
    InvalidChar(char),
    #[error("code is too short")]
    TooShort,
    #[error("unsupported puzzle code version {0}")]
    Version(u8),
    #[error("code length does not match its params")]
    Length,
    #[error("non-zero padding bits")]
    Padding,
    #[error("cell value {0} is out of range")]
    CellValue(u8),
    #[error("tube {0} has a unit above an empty cell")]
    Floating(usize),
    #[error(transparent)]
    State(#[from] StateError),
}

fn bits_per_cell(params: Params) -> u32 {
    u32::from(params.n_colors).bit_width()
}

fn n_cells(params: Params) -> usize {
    params.n_tubes() * usize::from(params.capacity)
}

/// Encodes a state as a puzzle code.
pub fn encode(s: &State) -> String {
    let p = s.params();
    let bits = bits_per_cell(p);
    let mut bytes = vec![VERSION, p.n_colors, p.capacity, p.n_empty];
    let mut writer = BitWriter::default();
    for i in 0..s.n_tubes() {
        for &c in s.padded_tube(i) {
            writer.push(if c == EMPTY { 0 } else { u32::from(c) + 1 }, bits);
        }
    }
    bytes.extend(writer.finish());
    base32_encode(&bytes)
}

/// Decodes a puzzle code. Case-insensitive; `I`/`L` read as `1`, `O` as `0`, and `-` is ignored.
///
/// # Errors
///
/// Malformed codes and codes that do not describe a valid [`State`].
pub fn decode(code: &str) -> Result<State, PuzzleCodeError> {
    let bytes = base32_decode(code)?;
    let [version, n_colors, capacity, n_empty, body @ ..] = bytes.as_slice() else {
        return Err(PuzzleCodeError::TooShort);
    };
    if *version != VERSION {
        return Err(PuzzleCodeError::Version(*version));
    }
    let params = Params {
        n_colors: *n_colors,
        capacity: *capacity,
        n_empty: *n_empty,
    };
    params.validate().map_err(StateError::from)?;
    let bits = bits_per_cell(params);
    let total_bits = n_cells(params) * bits as usize;
    if body.len() != total_bits.div_ceil(8) {
        return Err(PuzzleCodeError::Length);
    }
    let mut reader = BitReader::new(body);
    let mut tubes = Vec::with_capacity(params.n_tubes());
    for i in 0..params.n_tubes() {
        let mut tube = Vec::new();
        let mut ended = false;
        for _ in 0..params.capacity {
            let v = reader.read(bits);
            if v == 0 {
                ended = true;
            } else if ended {
                return Err(PuzzleCodeError::Floating(i));
            } else {
                let c = low_byte(v - 1); // v < 32
                if c >= params.n_colors {
                    return Err(PuzzleCodeError::CellValue(c + 1));
                }
                tube.push(c);
            }
        }
        tubes.push(tube);
    }
    if reader.rest() != 0 {
        return Err(PuzzleCodeError::Padding);
    }
    Ok(State::from_tubes(params, &tubes)?)
}

#[derive(Default)]
struct BitWriter {
    bytes: Vec<u8>,
    acc: u32,
    n: u32,
}

impl BitWriter {
    fn push(&mut self, value: u32, bits: u32) {
        self.acc = (self.acc << bits) | value;
        self.n += bits;
        while self.n >= 8 {
            self.n -= 8;
            self.bytes.push(low_byte(self.acc >> self.n));
        }
        self.acc &= (1 << self.n) - 1;
    }

    fn finish(mut self) -> Vec<u8> {
        if self.n > 0 {
            self.bytes.push(low_byte(self.acc << (8 - self.n)));
        }
        self.bytes
    }
}

const fn low_byte(x: u32) -> u8 {
    (x & 0xFF) as u8
}

struct BitReader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> BitReader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn read(&mut self, bits: u32) -> u32 {
        let mut v = 0;
        for _ in 0..bits {
            let bit = (self.bytes[self.pos / 8] >> (7 - self.pos % 8)) & 1;
            v = (v << 1) | u32::from(bit);
            self.pos += 1;
        }
        v
    }

    /// The remaining (padding) bits as a number.
    fn rest(&mut self) -> u32 {
        let left = self.bytes.len() * 8 - self.pos;
        self.read(u32::try_from(left).expect("fewer than 8 padding bits"))
    }
}

fn base32_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity((bytes.len() * 8).div_ceil(5));
    let (mut acc, mut n) = (0u32, 0u32);
    for &b in bytes {
        acc = (acc << 8) | u32::from(b);
        n += 8;
        while n >= 5 {
            n -= 5;
            out.push(char::from(ALPHABET[((acc >> n) & 31) as usize]));
        }
        acc &= (1 << n) - 1;
    }
    if n > 0 {
        out.push(char::from(ALPHABET[((acc << (5 - n)) & 31) as usize]));
    }
    out
}

fn base32_value(ch: char) -> Option<u32> {
    let up = ch.to_ascii_uppercase();
    let up = match up {
        'I' | 'L' => '1',
        'O' => '0',
        other => other,
    };
    ALPHABET
        .iter()
        .position(|&a| char::from(a) == up)
        .map(|i| u32::try_from(i).expect("alphabet index"))
}

fn base32_decode(code: &str) -> Result<Vec<u8>, PuzzleCodeError> {
    let mut out = Vec::with_capacity(code.len() * 5 / 8);
    let (mut acc, mut n) = (0u32, 0u32);
    for ch in code.chars().filter(|&c| c != '-') {
        let v = base32_value(ch).ok_or(PuzzleCodeError::InvalidChar(ch))?;
        acc = (acc << 5) | v;
        n += 5;
        if n >= 8 {
            n -= 8;
            out.push(low_byte(acc >> n));
            acc &= (1 << n) - 1;
        }
    }
    // Leftover bits are base32 padding and must be zero.
    if acc != 0 {
        return Err(PuzzleCodeError::Padding);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: Params = Params {
        n_colors: 3,
        capacity: 3,
        n_empty: 1,
    };

    #[test]
    fn round_trip_and_known_code() {
        let s = State::from_tubes(P, &[&[0, 1, 1][..], &[1, 0], &[2, 2, 2], &[0]]).unwrap();
        let code = encode(&s);
        assert_eq!(decode(&code), Ok(s));
        assert_eq!(decode(&code.to_lowercase()), Ok(s));
        // 4 header bytes + 12 cells * 2 bits = 3 bytes -> 7 bytes -> 12 characters.
        assert_eq!(code.len(), 12);
        let solved = State::solved(P).unwrap();
        assert_eq!(decode(&encode(&solved)), Ok(solved));
    }

    #[test]
    fn base32_basics() {
        assert_eq!(base32_encode(&[]), "");
        assert_eq!(base32_encode(&[0xFF]), "ZW");
        assert_eq!(base32_decode("ZW"), Ok(vec![0xFF]));
        assert_eq!(base32_decode("zw"), Ok(vec![0xFF]));
        assert_eq!(base32_decode("Z-W"), Ok(vec![0xFF]));
        assert_eq!(base32_decode("ZZ"), Err(PuzzleCodeError::Padding));
        assert_eq!(base32_decode("OI"), base32_decode("01"));
        assert_eq!(base32_decode("U"), Err(PuzzleCodeError::InvalidChar('U')));
    }

    #[test]
    fn rejects_malformed() {
        let s = State::solved(P).unwrap();
        let mut bytes = base32_decode(&encode(&s)).unwrap();
        assert_eq!(decode(""), Err(PuzzleCodeError::TooShort));
        bytes[0] = 2;
        assert_eq!(
            decode(&base32_encode(&bytes)),
            Err(PuzzleCodeError::Version(2))
        );
        bytes[0] = VERSION;
        let mut short = bytes.clone();
        short.pop();
        assert_eq!(decode(&base32_encode(&short)), Err(PuzzleCodeError::Length));
        // Tube 0 = [empty, 0, 0]: a unit above an empty cell.
        let mut floating = bytes.clone();
        floating[4] = 0b0001_0100;
        assert_eq!(
            decode(&base32_encode(&floating)),
            Err(PuzzleCodeError::Floating(0))
        );
        // Cell value 3 + 1 with 3 colors.
        let mut bad = bytes.clone();
        bad[4] = 0b1100_0000 | (bad[4] & 0x3F);
        assert!(matches!(
            decode(&base32_encode(&bad)),
            Err(PuzzleCodeError::CellValue(_) | PuzzleCodeError::State(_))
        ));
        let mut params = bytes;
        params[1] = 0;
        assert!(matches!(
            decode(&base32_encode(&params)),
            Err(PuzzleCodeError::State(StateError::Params(_)))
        ));
    }
}
