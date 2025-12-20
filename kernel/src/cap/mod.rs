pub mod capability;
pub mod captype;
pub mod cnode;
pub mod invoke;
pub mod method;

pub use capability::Capability;
pub use captype::CapType;
pub use cnode::{CNode, Slot};

pub type CapPtr = usize;

/// 能力权限位 (Bitmask)
pub mod rights {
    // 通用权限
    pub const READ: u8 = 1 << 0; // 允许读取寄存器/内存
    pub const WRITE: u8 = 1 << 1; // 允许写入寄存器/内存
    pub const GRANT: u8 = 1 << 2; // 允许传递此 Cap (Grant)

    // IPC 专属权限
    pub const SEND: u8 = 1 << 3; // 允许发送消息 (sys_send)
    pub const RECV: u8 = 1 << 4; // 允许接收消息 (sys_recv)
    pub const CALL: u8 = 1 << 5; // 允许调用对象方法 (sys_invoke)

    // 组合权限
    pub const ALL: u8 = 0xFF;
    pub const RW: u8 = READ | WRITE;
    pub const MASTER: u8 = ALL;
}
