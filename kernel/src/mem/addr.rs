use super::PGSIZE;
use core::ops::{Add, AddAssign, Sub, SubAssign};

pub const VA_MAX: usize = 1 << 38; // 256 GiB 虚拟地址空间上限
pub const EMPTY_VA: usize = 0x0; // 空虚拟地址
pub const TRAMPOLINE_VA: usize = VA_MAX - PGSIZE; // Trampoline 映射地址
pub const TRAPFRAME_VA: usize = TRAMPOLINE_VA - PGSIZE; // Trapframe 映射地址
pub const UTCB_VA: usize = TRAPFRAME_VA - PGSIZE; // UTCB 映射地址 0x3FFFFFD000
pub const BOOTINFO_VA: usize = UTCB_VA - PGSIZE; // BootInfo 映射地址
pub const INITRD_VA: usize = 0x3000_0000; // Initrd 映射地址 (Root Task)

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct PhysAddr(usize);

impl PhysAddr {
    pub const fn from(addr: usize) -> Self {
        Self(addr as usize)
    }
    pub const fn as_usize(&self) -> usize {
        self.0
    }
    pub fn to_va(&self) -> VirtAddr {
        VirtAddr(self.0)
    }
    pub const fn to_ppn(&self) -> PPN {
        PPN(self.0 >> 12)
    }
    pub fn as_mut_ptr<T>(&self) -> *mut T {
        self.0 as *mut T
    }
    pub fn as_ptr<T>(&self) -> *const T {
        self.0 as *const T
    }
    pub fn as_ref<T>(&self) -> &'static T {
        unsafe { &*(self.as_ptr::<T>()) }
    }
    pub fn as_mut<T>(&self) -> &'static mut T {
        unsafe { &mut *(self.as_mut_ptr::<T>()) }
    }
    pub const fn null() -> Self {
        Self(0)
    }
    pub fn align_down(&self, align: usize) -> Self {
        PhysAddr((self.0 + align - 1) & !(align - 1))
    }
    pub fn align_up(&self, align: usize) -> Self {
        PhysAddr((self.0 + align - 1) & !(align - 1))
    }
    pub fn is_aligned(&self, align: usize) -> bool {
        self.0 % align == 0
    }
}

impl Add for PhysAddr {
    type Output = PhysAddr;
    fn add(self, rhs: PhysAddr) -> PhysAddr {
        PhysAddr(self.0 + rhs.0)
    }
}
impl Add<usize> for PhysAddr {
    type Output = PhysAddr;
    fn add(self, rhs: usize) -> PhysAddr {
        PhysAddr(self.0 + rhs)
    }
}
impl Sub for PhysAddr {
    type Output = PhysAddr;
    fn sub(self, rhs: PhysAddr) -> PhysAddr {
        PhysAddr(self.0 - rhs.0)
    }
}
impl Sub<usize> for PhysAddr {
    type Output = PhysAddr;
    fn sub(self, rhs: usize) -> PhysAddr {
        PhysAddr(self.0 - rhs)
    }
}
impl AddAssign<usize> for PhysAddr {
    fn add_assign(&mut self, rhs: usize) {
        self.0 += rhs;
    }
}
impl SubAssign<usize> for PhysAddr {
    fn sub_assign(&mut self, rhs: usize) {
        self.0 -= rhs;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd)]
pub struct VirtAddr(usize);
impl VirtAddr {
    pub const fn from(addr: usize) -> Self {
        assert!(addr < VA_MAX, "VirtAddr out of range");
        Self(addr as usize)
    }
    pub const fn as_usize(&self) -> usize {
        self.0
    }
    pub fn to_pa(&self) -> PhysAddr {
        PhysAddr(self.0)
    }
    pub const fn vpn(&self) -> [VPN; 3] {
        [VPN((self.0 >> 12) & 0x1FF), VPN((self.0 >> 21) & 0x1FF), VPN((self.0 >> 30) & 0x1FF)]
    }
    pub fn as_mut_ptr<T>(&self) -> *mut T {
        self.0 as *mut T
    }
    pub fn as_ptr<T>(&self) -> *const T {
        self.0 as *const T
    }
    pub fn as_ref<T>(&self) -> &'static T {
        unsafe { &*(self.as_ptr::<T>()) }
    }
    pub fn as_mut<T>(&self) -> &'static mut T {
        unsafe { &mut *(self.as_mut_ptr::<T>()) }
    }
    pub const fn null() -> Self {
        Self(0)
    }
    pub fn align_down(&self, align: usize) -> Self {
        VirtAddr((self.0 + align - 1) & !(align - 1))
    }
    pub fn align_up(&self, align: usize) -> Self {
        VirtAddr((self.0 + align - 1) & !(align - 1))
    }
    pub const fn max() -> Self {
        Self(VA_MAX - 1)
    }
    pub fn is_aligned(&self, align: usize) -> bool {
        self.0 % align == 0
    }
}

impl Add for VirtAddr {
    type Output = VirtAddr;
    fn add(self, rhs: VirtAddr) -> VirtAddr {
        VirtAddr(self.0 + rhs.0)
    }
}
impl Add<usize> for VirtAddr {
    type Output = VirtAddr;
    fn add(self, rhs: usize) -> VirtAddr {
        VirtAddr(self.0 + rhs)
    }
}
impl Sub for VirtAddr {
    type Output = VirtAddr;
    fn sub(self, rhs: VirtAddr) -> VirtAddr {
        VirtAddr(self.0 - rhs.0)
    }
}
impl Sub<usize> for VirtAddr {
    type Output = VirtAddr;
    fn sub(self, rhs: usize) -> VirtAddr {
        VirtAddr(self.0 - rhs)
    }
}
impl AddAssign<usize> for VirtAddr {
    fn add_assign(&mut self, rhs: usize) {
        self.0 += rhs;
    }
}
impl SubAssign<usize> for VirtAddr {
    fn sub_assign(&mut self, rhs: usize) {
        self.0 -= rhs;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd)]
pub struct PPN(usize);
impl PPN {
    pub const fn from(ppn: usize) -> Self {
        assert!(ppn < (VA_MAX >> 12), "PPN out of range");
        Self(ppn)
    }
    pub const fn as_usize(&self) -> usize {
        self.0
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd)]
pub struct VPN(usize);
impl VPN {
    pub const fn from(vpn: usize) -> Self {
        assert!(vpn < 0x200, "VPN out of range");
        Self(vpn)
    }
    pub const fn as_usize(&self) -> usize {
        self.0
    }
}
