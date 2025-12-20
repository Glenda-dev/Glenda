/// 消息标签 (Message Tag) 结构
/// 用于描述 IPC 消息的元数据
#[derive(Debug, Clone, Copy)]
pub struct MsgTag(pub usize);

impl MsgTag {
    pub const FLAG_HAS_CAP: usize = 1 << 4;

    pub fn new(label: usize, length: usize) -> Self {
        // Label: bits 16+, Length: bits 0-3
        Self((label << 16) | (length & 0xF))
    }

    pub fn label(&self) -> usize {
        self.0 >> 16
    }

    pub fn length(&self) -> usize {
        self.0 & 0xF
    }

    pub fn has_cap(&self) -> bool {
        (self.0 & Self::FLAG_HAS_CAP) != 0
    }

    pub fn set_has_cap(&mut self) {
        self.0 |= Self::FLAG_HAS_CAP;
    }
}

pub mod label {
    pub const FAULT: usize = 0xFFFF;
    pub const NOTIFY: usize = 0xFFFE;
}
