use super::capability::DATA_SHIFT;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Badge(usize);

const BADGE_BITS: usize = 64 - DATA_SHIFT;
pub const BADGE_MASK: usize = (1 << BADGE_BITS) - 1;

impl Badge {
    pub const fn from(value: usize) -> Self {
        Badge(value & BADGE_MASK)
    }
    pub fn is_null(&self) -> bool {
        self.0 == 0
    }
    pub fn get(&self) -> usize {
        self.0
    }
    pub const fn null() -> Self {
        Badge(0)
    }
}
