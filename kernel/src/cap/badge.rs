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

    pub fn bits(&self) -> usize {
        self.0
    }
}

impl core::ops::BitOr for Badge {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Badge::from(self.0 | rhs.0)
    }
}

impl core::ops::BitAnd for Badge {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self::Output {
        Badge::from(self.0 & rhs.0)
    }
}
