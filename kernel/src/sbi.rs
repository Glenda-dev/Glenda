#![allow(dead_code)]

// TODO: Refactor this

unsafe extern "C" {
    fn sbi_set_timer_asm(stime_value: u64) -> isize;
}

pub fn set_timer(stime_value: u64) -> Result<(), isize> {
    let error = unsafe { sbi_set_timer_asm(stime_value) };
    if error == 0 { Ok(()) } else { Err(error) }
}
