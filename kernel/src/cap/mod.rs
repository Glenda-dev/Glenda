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
pub use cnode::{CNode, CapPtr};

use crate::ipc::MAX_MRS;
use bitflags::bitflags;
use core::fmt::Display;

pub type Args = [usize; MAX_MRS];

bitflags! {
    #[derive(Clone,Copy)]
    pub struct Rights: u8 {
        const READ = 1 << 0; // 允许读取寄存器/内存
        const WRITE = 1 << 1; // 允许写入寄存器/内存
        const GRANT = 1 << 2; // 允许传递此 Cap (Grant)
        const SEND = 1 << 3; // 允许发送消息 (sys_send)
        const RECV = 1 << 4; // 允许接收消息 (sys_recv)
        const CALL = 1 << 5; // 允许调用对象方法 (sys_invoke)
        const EXECUTE = 1 << 6; // 允许执行 (仅用于 TCB)
        const ALL = 0xFF;
    }
}

impl Display for Rights {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut first = true;
        let perms = [
            (Rights::READ, "R"),
            (Rights::WRITE, "W"),
            (Rights::GRANT, "G"),
            (Rights::SEND, "S"),
            (Rights::RECV, "R"),
            (Rights::CALL, "C"),
        ];
        for (bit, name) in perms.iter() {
            if self.contains(*bit) {
                if !first {
                    write!(f, "|")?;
                }
                write!(f, "{}", name)?;
                first = false;
            }
        }
        if first {
            write!(f, "NONE")?;
        }
        Ok(())
    }
}
