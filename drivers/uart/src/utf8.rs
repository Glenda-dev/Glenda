const LINEBUF_CAP: usize = 256;

struct ConsoleEcho {
    decoder: Utf8Decoder,
    widths: [u8; LINEBUF_CAP],
    len: usize,
}

impl ConsoleEcho {
    const fn new() -> Self {
        Self { decoder: Utf8Decoder::new(), widths: [0; LINEBUF_CAP], len: 0 }
    }
    fn clear_line(&mut self) {
        self.len = 0;
    }
    fn push_width(&mut self, w: u8) {
        if w == 0 {
            return;
        }
        if self.len < LINEBUF_CAP {
            self.widths[self.len] = w;
            self.len += 1;
        } else {
            let mut i = 1;
            while i < LINEBUF_CAP {
                self.widths[i - 1] = self.widths[i];
                i += 1;
            }
            self.widths[LINEBUF_CAP - 1] = w;
        }
    }
    fn pop_width(&mut self) -> Option<u8> {
        if self.len == 0 {
            None
        } else {
            self.len -= 1;
            Some(self.widths[self.len])
        }
    }
}

struct Utf8Decoder {
    buf: [u8; 4],
    len: u8, // buffered length (0..=4)
    need: u8,
}

impl Utf8Decoder {
    const fn new() -> Self {
        Self { buf: [0; 4], len: 0, need: 0 }
    }
    fn clear(&mut self) {
        self.len = 0;
        self.need = 0;
    }
    fn has_pending(&self) -> bool {
        self.len > 0
    }
    fn push(&mut self, b: u8) -> Utf8PushResult {
        if self.len == 0 {
            let need = utf8_expected_len(b);
            if need == 0 {
                return Utf8PushResult::Invalid;
            }
            self.buf[0] = b;
            self.len = 1;
            self.need = need;
            return Utf8PushResult::Pending;
        } else {
            if (b & 0b1100_0000) != 0b1000_0000 {
                // invalid continuation
                self.clear();
                return Utf8PushResult::Invalid;
            }
            if (self.len as usize) < self.buf.len() {
                self.buf[self.len as usize] = b;
            }
            self.len += 1;
            if self.len == self.need {
                let slice = &self.buf[..self.len as usize];
                if let Ok(s) = core::str::from_utf8(slice) {
                    let mut it = s.chars();
                    let c = it.next().unwrap_or('\u{FFFD}');
                    if it.next().is_none() {
                        self.clear();
                        return Utf8PushResult::Completed(c);
                    }
                }
                self.clear();
                return Utf8PushResult::Invalid;
            }
            Utf8PushResult::Pending
        }
    }
}

enum Utf8PushResult {
    Pending,
    Completed(char),
    Invalid,
}

fn utf8_expected_len(b: u8) -> u8 {
    match b {
        0xC2..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF4 => 4,
        _ => 0,
    }
}

static CONSOLE_ECHO: Mutex<ConsoleEcho> = Mutex::new(ConsoleEcho::new());

// TODO: 组合音标似乎是零宽，目前退格没法正常显示
fn char_display_width(c: char) -> u8 {
    if c.is_ascii() {
        return 1;
    }
    let u = c as u32;
    const RANGES_2: &[(u32, u32)] = &[
        (0x1100, 0x115F),   // Hangul Jamo init
        (0x2329, 0x232A),   // angle brackets
        (0x2E80, 0xA4CF),   // CJK Radicals..Yi
        (0xAC00, 0xD7A3),   // Hangul Syllables
        (0xF900, 0xFAFF),   // CJK Compatibility Ideographs
        (0xFE10, 0xFE19),   // Vertical forms
        (0xFE30, 0xFE6F),   // CJK Compatibility Forms
        (0xFF00, 0xFF60),   // Fullwidth Forms
        (0xFFE0, 0xFFE6),   // Fullwidth symbol variants
        (0x1F300, 0x1F64F), // Emoji
        (0x1F900, 0x1F9FF), // Emoji
        (0x20000, 0x2FFFD), // CJK Unified Ideographs Ext
        (0x30000, 0x3FFFD), // CJK Unified Ideographs Ext
    ];
    for &(lo, hi) in RANGES_2 {
        if u >= lo && u <= hi {
            return 2;
        }
    }
    1
}
