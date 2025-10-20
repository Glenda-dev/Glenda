#![allow(dead_code)]
use super::clint::{get_mtime, get_mtimecmp, set_mtimecmp};
use super::vector::timer_vector_base;
use riscv::register::mtvec::{self, Mtvec};
use riscv::register::{mie, mscratch, mstatus};
use spin::Mutex;
struct SysTimer {
    ticks: usize,
}

static mut MSCRATCH: [[usize; 5]; 8] = [[0; 5]; 8];
const INTERVAL: usize = 1000000; // 100ms
static SYS_TIMER: Mutex<SysTimer> = Mutex::new(SysTimer { ticks: 0 });

pub fn init(hartid: usize) {
    // 设置初始值 cmp_time = cur_time + time_interval
    set_mtimecmp(hartid, get_mtime() + INTERVAL);
    unsafe {
        // cur_mscratch 指向当前 CPU 的 msrcatch 数组
        let cur_mscratch = &mut MSCRATCH[hartid];
        // cur_mscratch [1] [2] [3] 先空着, 在 trap.S 里使用
        cur_mscratch[3] = get_mtimecmp(hartid); // CLINT_MTIMECMP 地址
        cur_mscratch[4] = INTERVAL; // INTERVAL
        mscratch::write(cur_mscratch.as_mut_ptr() as usize);
        let timer_vec = Mtvec::new(timer_vector_base as usize, mtvec::TrapMode::Vectored);
        Mtvec::new(timer_vector_base as usize, mtvec::TrapMode::Vectored);
        mtvec::write(timer_vec);
        // 打开 M-mode 中断总开关
        mstatus::set_mie();
        // 打开 M-mode 时钟中断分开关
        mie::set_mtimer();
    }
}
pub fn create() {
    let mut sys_timer = SYS_TIMER.lock();
    sys_timer.ticks = 0;
}
pub fn update() {
    let mut sys_timer = SYS_TIMER.lock();
    sys_timer.ticks += 1;
}
pub fn get_ticks() -> usize {
    let sys_timer = SYS_TIMER.lock();
    sys_timer.ticks
}
