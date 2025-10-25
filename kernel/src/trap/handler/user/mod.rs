pub mod syscall;

use super::super::TrapFrame;

/// U-mode 陷阱处理函数
/// 在 kernel_vector 汇编代码中被调用
#[unsafe(no_mangle)]
pub extern "C" fn trap_user_handler(ctx: &mut TrapFrame) {}

#[unsafe(no_mangle)]
pub extern "C" fn trap_user_return(ctx: &mut TrapFrame) {}
