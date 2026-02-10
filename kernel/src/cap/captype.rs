use num_enum::FromPrimitive;

/// 内核对象类型
/// 仅用于标识 Capability 的类型，不再携带数据

#[derive(Debug, Clone, Copy, FromPrimitive, PartialEq, Eq, PartialOrd, Ord)]
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
    #[num_enum(default)]
    Unknown = 255,
}
