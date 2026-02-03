// TODO: Check address validity
use crate::hal::mem::VA_MAX;
use core::fmt::{Debug, Display};
use core::ops::{Add, AddAssign, Sub, SubAssign};

#[derive(Clone, Copy, Eq, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct PhysAddr(usize);

impl PhysAddr {
    pub const fn from(addr: usize) -> Self {
        Self(addr as usize)
    }
    pub const fn as_usize(&self) -> usize {
        self.0
    }
    pub const fn null() -> Self {
        Self(0)
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
    pub fn align_down(&self, align: usize) -> Self {
        PhysAddr(self.0 & !(align - 1))
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

impl Display for PhysAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:#x}", self.0)
    }
}

impl Debug for PhysAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:#x}", self.0)
    }
}

#[derive(Clone, Copy, Eq, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct VirtAddr(usize);
impl VirtAddr {
    pub const fn from(addr: usize) -> Self {
        Self(addr as usize)
    }
    pub const fn as_usize(&self) -> usize {
        self.0
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
        VirtAddr(self.0 & !(align - 1))
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
impl Display for VirtAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:#x}", self.0)
    }
}
impl Debug for VirtAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:#x}", self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd)]
pub struct PPN(usize);
impl PPN {
    pub const fn from(ppn: usize) -> Self {
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
        Self(vpn)
    }
    pub const fn as_usize(&self) -> usize {
        self.0
    }
}
