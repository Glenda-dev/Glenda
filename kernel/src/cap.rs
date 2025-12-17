// kernel/src/cap/mod.rs

use crate::mem::PhysAddr;
use alloc::collections::BTreeMap;

pub type CapPtr = usize;

/// 能力权限位
pub mod rights {
    pub const READ: u8 = 1 << 0;
    pub const WRITE: u8 = 1 << 1;
    pub const GRANT: u8 = 1 << 2; // 允许传递此 Cap
    pub const CALL: u8 = 1 << 3; // 允许 Invoke
}

/// 内核对象类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapType {
    Empty,
    /// 未类型化内存，可用于 Retype 生成其他对象
    Untyped {
        start: PhysAddr,
        size: usize,
    },
    /// 线程控制块 (Thread Control Block)，这里暂时关联到 PID
    TCB {
        pid: usize,
    },
    /// IPC 通信端点
    Endpoint {
        id: usize,
    },
    /// 物理页帧，可映射到地址空间
    Frame {
        start: PhysAddr,
    },
    /// 页表
    PageTable {
        start: PhysAddr,
    },
    /// 中断处理权限
    IrqHandler {
        irq: usize,
    },
    /// CSpace 节点 (用于构建层级 CSpace，这里简化处理)
    CNode {
        start: PhysAddr,
        size: usize,
    },
}

/// 能力 (Capability)
/// 包含对象引用、权限和 Badge
#[derive(Debug, Clone, Copy)]
pub struct Capability {
    pub object: CapType,
    pub badge: Option<usize>, // Badge 用于服务端识别客户端身份
    pub rights: u8,
}

impl Capability {
    pub const fn new(object: CapType, rights: u8) -> Self {
        Self { object, badge: None, rights }
    }

    /// Mint 操作：创建一个新的 Cap，可以附加 Badge
    pub fn mint(&self, badge: Option<usize>) -> Self {
        Self {
            object: self.object,
            badge: badge.or(self.badge), // 如果已有 Badge 则保留，否则使用新的
            rights: self.rights,
        }
    }
}

/// 能力空间 (CSpace)
/// 每个进程拥有一个 CSpace，用于存储它拥有的 Cap
/// 在完整微内核中通常是 Radix Tree 结构的 CNode 树，这里先用 BTreeMap 模拟扁平结构
pub struct CSpace {
    slots: BTreeMap<usize, Capability>,
}

impl CSpace {
    pub const fn new() -> Self {
        Self { slots: BTreeMap::new() }
    }

    pub fn get(&self, cptr: CapPtr) -> Option<&Capability> {
        self.slots.get(&cptr)
    }

    pub fn insert(&mut self, cptr: CapPtr, cap: Capability) {
        self.slots.insert(cptr, cap);
    }

    pub fn remove(&mut self, cptr: CapPtr) -> Option<Capability> {
        self.slots.remove(&cptr)
    }

    /// 查找空闲槽位 (简单实现)
    pub fn alloc_slot(&self) -> CapPtr {
        let mut i = 0;
        while self.slots.contains_key(&i) {
            i += 1;
        }
        i
    }
}
