use core::fmt::Display;

/// 架构无关的 Trap 原因枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrapCause {
    /// 异常 (Exception)
    Exception(TrapException),
    /// 中断 (Interrupt)
    Interrupt(TrapInterrupt),
    /// 未知原因 (Unknown)
    ///
    /// 包含原始的 cause 寄存器值
    Unknown(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum TrapException {
    Syscall,
    VirtualSupervisorSyscall,
    PageFault,
    GuestPageFault,
    VirtualInstruction,
    IllegalInstruction,
    Breakpoint,
    AccessFault,
    AccessMisaligned,
    Unknown(usize),
}

impl TrapException {
    pub fn as_usize(&self) -> usize {
        match self {
            TrapException::Syscall => 1,
            TrapException::VirtualSupervisorSyscall => 7,
            TrapException::PageFault => 2, // Using Instruction Page Fault code as representative
            TrapException::GuestPageFault => 8,
            TrapException::VirtualInstruction => 9,
            TrapException::IllegalInstruction => 3,
            TrapException::Breakpoint => 4,
            TrapException::AccessFault => 5, // Using Load Access Fault code as representative
            TrapException::AccessMisaligned => 6, // Using Load Address Misaligned code as representative
            TrapException::Unknown(code) => *code,
        }
    }
}

impl Display for TrapException {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TrapException::Syscall => write!(f, "Syscall"),
            TrapException::VirtualSupervisorSyscall => write!(f, "VirtualSupervisorSyscall"),
            TrapException::PageFault => write!(f, "PageFault"),
            TrapException::GuestPageFault => write!(f, "GuestPageFault"),
            TrapException::VirtualInstruction => write!(f, "VirtualInstruction"),
            TrapException::IllegalInstruction => write!(f, "IllegalInstruction"),
            TrapException::Breakpoint => write!(f, "Breakpoint"),
            TrapException::AccessFault => write!(f, "AccessFault"),
            TrapException::AccessMisaligned => write!(f, "AccessMisaligned"),
            TrapException::Unknown(code) => write!(f, "Unknown({})", code),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum TrapInterrupt {
    Timer,
    VirtualSupervisorTimer,
    External,
    VirtualSupervisorExternal,
    Software,
    VirtualSupervisorSoftware,
    Unknown(usize),
}

impl TrapInterrupt {
    pub fn as_usize(&self) -> usize {
        match self {
            TrapInterrupt::Timer => 1,    // Supervisor Timer Interrupt
            TrapInterrupt::VirtualSupervisorTimer => 4,
            TrapInterrupt::External => 2, // Supervisor External Interrupt
            TrapInterrupt::VirtualSupervisorExternal => 5,
            TrapInterrupt::Software => 3, // Supervisor Software Interrupt
            TrapInterrupt::VirtualSupervisorSoftware => 6,
            TrapInterrupt::Unknown(code) => *code,
        }
    }
}

impl Display for TrapInterrupt {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TrapInterrupt::Timer => write!(f, "Timer Interrupt"),
            TrapInterrupt::VirtualSupervisorTimer => write!(f, "VirtualSupervisorTimer Interrupt"),
            TrapInterrupt::External => write!(f, "External Interrupt"),
            TrapInterrupt::VirtualSupervisorExternal => {
                write!(f, "VirtualSupervisorExternal Interrupt")
            }
            TrapInterrupt::Software => write!(f, "Software Interrupt"),
            TrapInterrupt::VirtualSupervisorSoftware => {
                write!(f, "VirtualSupervisorSoftware Interrupt")
            }
            TrapInterrupt::Unknown(code) => write!(f, "Unknown({})", code),
        }
    }
}
