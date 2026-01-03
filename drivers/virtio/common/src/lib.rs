#![no_std]

pub mod consts;
pub mod queue;
pub mod transport;

pub use transport::VirtIOTransport;

#[derive(Debug)]
pub enum VirtIOError {
    DeviceNotFound,
    InvalidHeader,
    QueueTooSmall,
    OOM,
}

pub type Result<T> = core::result::Result<T, VirtIOError>;
