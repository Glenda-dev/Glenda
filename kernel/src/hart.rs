unsafe extern "C" {
    fn hart_get_tp() -> usize;
}

#[inline(always)]
pub fn getid() -> usize {
    unsafe { hart_get_tp() }
}
