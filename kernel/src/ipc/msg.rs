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

    #[cfg(target_pointer_width = "64")]
    pub fn new(proto: usize, label: usize, flags: MsgFlags) -> Self {
        Self((proto & 0xFFFFFFFF) << 32 | (label & 0xFFFF) << 16 | (flags.bits() & 0xFFFF))
    }

    #[cfg(target_pointer_width = "32")]
    pub fn new(proto: usize, label: usize, flags: MsgFlags) -> Self {
        Self((proto & 0xFFFF) << 16 | (label & 0xFF) << 8 | (flags.bits() & 0xFF))
    }

    pub fn as_usize(&self) -> usize {
        self.0
    }

    #[cfg(target_pointer_width = "64")]
    pub fn proto(&self) -> usize {
        (self.0 >> 32) & 0xFFFFFFFF
    }

    #[cfg(target_pointer_width = "32")]
    pub fn proto(&self) -> usize {
        (self.0 >> 16) & 0xFFFF
    }

    #[cfg(target_pointer_width = "64")]
    pub fn label(&self) -> usize {
        (self.0 >> 16) & 0xFFFF
    }

    #[cfg(target_pointer_width = "32")]
    pub fn label(&self) -> usize {
        (self.0 >> 8) & 0xFF
    }

    #[cfg(target_pointer_width = "64")]
    pub fn flags(&self) -> MsgFlags {
        MsgFlags::from_bits_truncate(self.0 & 0xFFFF)
    }

    #[cfg(target_pointer_width = "32")]
    pub fn flags(&self) -> MsgFlags {
        MsgFlags::from_bits_truncate(self.0 & 0xFF)
    }
}

bitflags! {
    #[derive(Clone, Copy)]
    pub struct MsgFlags: usize {
        const NONE = 0;
        const OK = 1 << 0;
        const ERROR = 1 << 1;
        const HAS_CAP = 1 << 2;
        const HAS_BUFFER = 1 << 3;
        const HAS_MRS = 1 << 4;
    }
}
