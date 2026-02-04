use core::mem::transmute;

/// 内核对象类型
/// 仅用于标识 Capability 的类型，不再携带数据
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum CapType {
    Empty = 0,
    Untyped = 1,
    TCB = 2,
    Endpoint = 3,
    Reply = 4,
    Frame = 5,
    PageTable = 6,
    CNode = 7,
    IrqHandler = 8,
    Kernel = 9,
    VSpace = 10,
}

impl CapType {
    #[inline(always)]
    pub const fn from(value: usize) -> Self {
        unsafe { transmute(value) }
    }
}
