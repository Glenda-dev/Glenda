use crate::hart;
use riscv::register::{sie, sscratch, sstatus};

pub fn init() {
    let hartid = hart::getid();
    unsafe {
        sscratch::write(hartid);
        sstatus::set_sie();
        sie::set_sext();
        sie::set_ssoft();
        sie::set_stimer();
        super::timer::start(hartid);
    }
}

/// 检查当前是否开启了 S 态中断
#[inline]
pub fn is_enabled() -> bool {
    sstatus::read().sie()
}

/// 关闭 S 态中断
#[inline]
pub fn disable() {
    unsafe { sstatus::clear_sie() };
}

/// 开启 S 态中断
#[inline]
pub fn enable() {
    unsafe { sstatus::set_sie() };
}
/// 进入中断上下文
pub fn enter() {
    let hart = hart::get();
    hart.nest_count += 1;
}
/// 退出中断上下文
pub fn exit() {
    let hart = hart::get();
    if hart.nest_count > 0 {
        hart.nest_count -= 1;
    }
}
