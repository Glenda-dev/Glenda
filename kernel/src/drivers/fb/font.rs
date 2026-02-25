use font8x8::UnicodeFonts;

pub const FONT_WIDTH: usize = 8;
pub const FONT_HEIGHT: usize = 8;

pub fn get_glyph(c: char) -> Option<[u8; 8]> {
    font8x8::BASIC_FONTS.get(c)
}
