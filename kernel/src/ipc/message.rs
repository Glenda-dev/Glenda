/// 消息标签 (Message Tag) 结构
/// 用于描述 IPC 消息的元数据
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
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
    // --- Kernel Protocols (Reserved High Values) ---

    /// 缺页异常 (Page Fault)
    /// 触发条件：用户态程序访问了无效的虚拟地址
    /// 消息内容：[fault_addr, instruction_addr, cause]
    pub const PAGE_FAULT: usize = 0xFFFF;

    /// 通用异常 (General Exception)
    /// 触发条件：非法指令、地址对齐错误等
    /// 消息内容：[cause, fault_val, instruction_addr]
    pub const EXCEPTION: usize = 0xFFFE;

    /// 未知系统调用 (Unknown Syscall)
    /// 触发条件：用户态调用了内核未定义的系统调用号
    /// 消息内容：[syscall_no, arg0, arg1, ...]
    pub const UNKNOWN_SYSCALL: usize = 0xFFFD;

    /// 能力错误 (Capability Fault)
    /// 触发条件：访问了无效的 Capability 或权限不足
    /// 消息内容：[cptr, error_code]
    pub const CAP_FAULT: usize = 0xFFFC;

    /// 中断通知 (Interrupt)
    /// 触发条件：硬件中断发生，内核转发给注册的处理程序
    /// 消息内容：通常为空，通过 Badge 区分
    pub const IRQ: usize = 0xFFFB;

    /// 异步通知 (Notification)
    /// 触发条件：ipc::notify
    /// 消息内容：通常为空，通过 Badge 传递事件位
    pub const NOTIFY: usize = 0xFFFA;

    // --- Legacy / Aliases ---

    /// 默认异常标签
    pub const FAULT: usize = EXCEPTION;
}
