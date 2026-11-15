//! A 3×5 pixel font: about as small as letters can get and still be read.
//!
//! Each glyph is five rows of three bits. In a row, `0b100` is the left pixel,
//! `0b010` the middle and `0b001` the right. Lower-case letters are drawn as
//! capitals, and characters we don't have come out as `?`.

pub const GLYPH_WIDTH: i32 = 3;
pub const GLYPH_HEIGHT: i32 = 5;
/// Distance from one character to the next (glyph plus one blank column).
pub const ADVANCE: i32 = 4;

/// Width in pixels of a line of text.
pub fn text_width(text: &str) -> i32 {
    let count = text.chars().count() as i32;
    if count == 0 { 0 } else { count * ADVANCE - 1 }
}

#[rustfmt::skip] // keep each glyph on one line so you can see its shape
pub fn glyph(c: char) -> [u8; 5] {
    match c.to_ascii_uppercase() {
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b001, 0b010, 0b010],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        'A' => [0b010, 0b101, 0b111, 0b101, 0b101],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'C' => [0b011, 0b100, 0b100, 0b100, 0b011],
        'D' => [0b110, 0b101, 0b101, 0b101, 0b110],
        'E' => [0b111, 0b100, 0b110, 0b100, 0b111],
        'F' => [0b111, 0b100, 0b110, 0b100, 0b100],
        'G' => [0b011, 0b100, 0b101, 0b101, 0b011],
        'H' => [0b101, 0b101, 0b111, 0b101, 0b101],
        'I' => [0b111, 0b010, 0b010, 0b010, 0b111],
        'J' => [0b001, 0b001, 0b001, 0b101, 0b010],
        'K' => [0b101, 0b101, 0b110, 0b101, 0b101],
        'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        'M' => [0b101, 0b111, 0b111, 0b101, 0b101],
        'N' => [0b110, 0b101, 0b101, 0b101, 0b101],
        'O' => [0b010, 0b101, 0b101, 0b101, 0b010],
        'P' => [0b110, 0b101, 0b110, 0b100, 0b100],
        'Q' => [0b010, 0b101, 0b101, 0b110, 0b011],
        'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
        'S' => [0b011, 0b100, 0b010, 0b001, 0b110],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        'U' => [0b101, 0b101, 0b101, 0b101, 0b111],
        'V' => [0b101, 0b101, 0b101, 0b101, 0b010],
        'W' => [0b101, 0b101, 0b111, 0b111, 0b101],
        'X' => [0b101, 0b101, 0b010, 0b101, 0b101],
        'Y' => [0b101, 0b101, 0b010, 0b010, 0b010],
        'Z' => [0b111, 0b001, 0b010, 0b100, 0b111],
        ' ' => [0b000, 0b000, 0b000, 0b000, 0b000],
        '.' => [0b000, 0b000, 0b000, 0b000, 0b010],
        ',' => [0b000, 0b000, 0b000, 0b010, 0b100],
        ':' => [0b000, 0b010, 0b000, 0b010, 0b000],
        '-' => [0b000, 0b000, 0b111, 0b000, 0b000],
        '+' => [0b000, 0b010, 0b111, 0b010, 0b000],
        '!' => [0b010, 0b010, 0b010, 0b000, 0b010],
        '%' => [0b101, 0b001, 0b010, 0b100, 0b101],
        '/' => [0b001, 0b001, 0b010, 0b100, 0b100],
        '<' => [0b001, 0b010, 0b100, 0b010, 0b001],
        '>' => [0b100, 0b010, 0b001, 0b010, 0b100],
        _ => [0b111, 0b001, 0b010, 0b000, 0b010], // '?'
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_and_digits_all_have_their_own_glyph() {
        let characters: Vec<char> = ('A'..='Z').chain('0'..='9').collect();
        for (i, a) in characters.iter().enumerate() {
            assert_ne!(glyph(*a), glyph('~'), "{a} falls back to '?'");
            for b in &characters[i + 1..] {
                assert_ne!(glyph(*a), glyph(*b), "{a} and {b} look the same");
            }
        }
        assert_eq!(glyph('a'), glyph('A'));
    }

    #[test]
    fn text_width_counts_the_gaps_between_letters() {
        assert_eq!(text_width(""), 0);
        assert_eq!(text_width("I"), 3);
        assert_eq!(text_width("FIRE"), 15);
    }
}
