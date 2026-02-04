pub struct ShellBuffer {
    buffer: [u8; 128],
    len: usize,
}

impl ShellBuffer {
    pub fn new() -> Self {
        Self { buffer: [0; 128], len: 0 }
    }

    pub fn push(&mut self, c: u8) {
        if self.len < self.buffer.len() {
            self.buffer[self.len] = c;
            self.len += 1;
        }
    }

    pub fn pop(&mut self) {
        if self.len > 0 {
            self.len -= 1;
        }
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buffer[..self.len]).unwrap_or("")
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    pub fn len(&self) -> usize {
        self.len
    }
}
