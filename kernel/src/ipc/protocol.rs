// 异常和故障

/// 系统调用 (Syscall)
/// 触发条件：用户态调用了系统调用号
/// 消息内容：[a0, a1, a2, a3, a4, a5, a6, a7]
pub const SYSCALL: usize = 0x01;

/// 缺页异常 (Page Fault)
/// 触发条件：用户态程序访问了无效的虚拟地址
// 消息格式: [addr, pc, cause]
pub const PAGE_FAULT: usize = 0x02;

/// 非法指令异常 (Illegal Instruction)
/// 触发条件：执行了未定义或非法的指令
/// 消息内容：[instruction, pc]
pub const ILLEGAL_INSTRUCTION: usize = 0x03;

/// 断点异常 (Breakpoint)
/// 触发条件：执行了断点指令
/// 消息内容：[pc]
pub const BREAKPOINT: usize = 0x04;

/// 访问失败异常 (Access Fault)
/// 触发条件：尝试访问受保护或不存在的内存区域
/// 消息内容：[addr, pc]
pub const ACCESS_FAULT: usize = 0x05;

/// 访问未对齐异常 (Access Misaligned)
/// 触发条件：尝试进行未对齐的内存访问
/// 消息内容：[addr, pc]
pub const ACCESS_MISALIGNED: usize = 0x06;

/// 默认异常标签
/// 用于未知或未分类的异常
/// 消息内容：[cause, value, pc]
pub const UNKNOWN_FAULT: usize = 0x07;

// 中断和通知

/// 中断通知 (Interrupt)
/// 触发条件：硬件中断发生，内核转发给注册的处理程序
/// 消息内容：通常为空，通过 Badge 区分
pub const IRQ: usize = 0x08;

/// 异步通知 (Notification)
/// 触发条件：ipc::notify
/// 消息内容：通常为空，通过 Badge 传递事件位
pub const NOTIFY: usize = 0x09;
