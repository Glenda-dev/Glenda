use riscv::register::sscratch;

#[inline(always)]
pub fn getid() -> usize {
    // 从 sscratch 读取在 inittraps_hart 写入的 hartid
    sscratch::read()
}
