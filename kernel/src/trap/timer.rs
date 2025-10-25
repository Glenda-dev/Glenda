#![allow(dead_code)]
use super::clint::{get_mtime, get_mtimecmp, set_mtimecmp};
use super::handler::vector::timer_vector_base;
use core::sync::atomic::{AtomicUsize, Ordering};
use riscv::register::mtvec::{self, Mtvec};
use riscv::register::time;
use riscv::register::{mie, mscratch, mstatus};

static mut MSCRATCH: [[usize; 5]; 8] = [[0; 5]; 8];
const INTERVAL: usize = 1000000; // 100ms

static SYS_TICKS: AtomicUsize = AtomicUsize::new(0);

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
    SYS_TICKS.store(0, Ordering::Relaxed);
}
pub fn update() {
    SYS_TICKS.fetch_add(1, Ordering::Relaxed);
}
pub fn get_ticks() -> usize {
    SYS_TICKS.load(Ordering::Relaxed)
}

#[inline(always)]
fn time_now() -> u64 {
    time::read() as u64
}

pub fn program_next_tick() {
    let next = time_now().wrapping_add(INTERVAL as u64);
    // FIXME: 错误处理
    let _ = crate::sbi::set_timer(next);
}

pub fn start(hartid: usize) {
    if hartid == 0 {
        program_next_tick();
    }
}
