/// Kernel Version Information
use crate::hal::ARCH;
pub const GIT_HASH: &str = env!("KERNEL_GIT_HASH");
pub const BUILD_TIME_STR: &str = env!("KERNEL_BUILD_TIME");

pub fn print() {
    crate::printk!(
        "Glenda Microkernel v{}.{}.{} (Commit: {}, Build: {}) on {}\n",
        env!("CARGO_PKG_VERSION_MAJOR"),
        env!("CARGO_PKG_VERSION_MINOR"),
        env!("CARGO_PKG_VERSION_PATCH"),
        GIT_HASH,
        BUILD_TIME_STR,
        ARCH
    );
}

pub fn get_version() -> u32 {
    let major: u32 = env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap_or(0);
    let minor: u32 = env!("CARGO_PKG_VERSION_MINOR").parse().unwrap_or(0);
    let patch: u32 = env!("CARGO_PKG_VERSION_PATCH").parse().unwrap_or(0);
    (major << 24) | (minor << 16) | patch
}

pub fn get_build_time_bytes() -> [u8; 64] {
    let bytes = BUILD_TIME_STR.as_bytes();
    let len = core::cmp::min(bytes.len(), 64);
    let mut buf = [0u8; 64];
    buf[..len].copy_from_slice(&bytes[..len]);
    buf
}

pub fn get_git_hash_bytes() -> [u8; 8] {
    let bytes = GIT_HASH.as_bytes();
    let len = core::cmp::min(bytes.len(), 8);
    let mut hash = [0u8; 8];
    hash[..len].copy_from_slice(&bytes[..len]);
    hash
}
