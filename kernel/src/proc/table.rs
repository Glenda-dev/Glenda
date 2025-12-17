// TODO: Deprecated
use super::process::Process;
use core::sync::atomic::AtomicUsize;
use spin::Mutex;

pub const NPROC: usize = 64;

pub static GLOBAL_PID: AtomicUsize = AtomicUsize::new(1);
pub static PROC_TABLE: Mutex<[Process; NPROC]> = Mutex::new([const { Process::new() }; NPROC]);
