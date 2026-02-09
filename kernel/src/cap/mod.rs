pub mod badge;
pub mod capability;
pub mod captype;
pub mod cnode;
pub mod invoke;
pub mod method;

pub use badge::Badge;
pub use capability::Capability;
pub use captype::CapType;
pub use cnode::CNODE_PAGES;
pub use cnode::{CNode, CapPtr, Slot};

use bitflags::bitflags;

bitflags! {
    #[derive(Clone,Copy,Debug)]
    pub struct Rights: u8 {
        const READ = 1 << 0; // 允许读取寄存器/内存
        const WRITE = 1 << 1; // 允许写入寄存器/内存
        const GRANT = 1 << 2; // 允许传递此 Cap (Grant)
        const SEND = 1 << 3; // 允许发送消息 (sys_send)
        const RECV = 1 << 4; // 允许接收消息 (sys_recv)
        const CALL = 1 << 5; // 允许调用对象方法 (sys_invoke)
        const EXECUTE = 1 << 6; // 允许执行 (仅用于 TCB)
        const CUSTOM = 1 << 7; // 允许特定操作
        const ALL = 0xFF;
    }
}
