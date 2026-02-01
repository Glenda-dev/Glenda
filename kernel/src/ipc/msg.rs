use bitflags::bitflags;

/// 消息标签 (Message Tag) 结构
/// 用于描述 IPC 消息的元数据
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct MsgTag(pub usize);

impl MsgTag {
    pub const fn empty() -> Self {
        Self(0)
    }

    pub fn new(proto: usize, label: usize, flags: MsgFlags) -> Self {
        Self((proto & 0xFFFF) << 24 | (label & 0xFFFF) << 16 | (flags.bits() & 0xFFFF))
    }

    pub fn as_usize(&self) -> usize {
        self.0
    }

    pub fn proto(&self) -> usize {
        (self.0 >> 24) & 0xFFFF
    }

    pub fn label(&self) -> usize {
        (self.0 >> 16) & 0xFFFF
    }

    pub fn flags(&self) -> MsgFlags {
        MsgFlags::from_bits_truncate(self.0 & 0xFFFF)
    }
}

bitflags! {
    pub struct MsgFlags: usize {
        const NONE = 0;
        const OK = 1 << 0;
        const ERROR = 1 << 1;
        const HAS_CAP = 1 << 2;
    }
}
