use num_enum::FromPrimitive;

/// 内核对象类型
/// 仅用于标识 Capability 的类型，不再携带数据
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive)]
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

impl CapType {
    pub const fn from_usize_const(val: usize) -> Self {
        match val {
            0 => CapType::Empty,
            1 => CapType::Untyped,
            2 => CapType::TCB,
            3 => CapType::Endpoint,
            4 => CapType::Reply,
            5 => CapType::Frame,
            6 => CapType::PageTable,
            7 => CapType::CNode,
            8 => CapType::IrqHandler,
            9 => CapType::Kernel,
            10 => CapType::VSpace,
            _ => CapType::Unknown,
        }
    }
}
