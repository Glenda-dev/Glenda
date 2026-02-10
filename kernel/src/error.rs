use num_enum::FromPrimitive;

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromPrimitive)]
pub enum Error {
    Success = 0,

    // --- General Errors (1-19) ---
    /// Generic unspecified error
    Generic = 1,
    /// Invalid arguments provided
    InvalidArgs = 2,
    /// Operation not supported
    NotSupported = 3,
    /// Feature not implemented
    NotImplemented = 4,
    /// Internal kernel error
    InternalError = 5,
    /// Operation timed out
    Timeout = 6,
    /// Operation interrupted
    Interrupted = 7,
    /// Resource temporarily unavailable (Try again)
    WouldBlock = 8,
    /// Component not initialized
    NotInitialized = 9,

    // --- Capability & Object Errors (20-39) ---
    /// Invalid capability reference (Empty or Wrong Type)
    InvalidCapability = 20,
    /// Permission denied
    PermissionDenied = 21,
    /// Invalid object type for operation
    InvalidType = 22,
    /// Invalid slot index
    InvalidSlot = 23,
    /// CNode is full / No slots available
    CNodeFull = 24,
    /// Target slot is not empty (for Mint/Copy/Move)
    SlotNotEmpty = 25,
    /// Invalid invocation method
    InvalidMethod = 26,

    // --- Memory & Address Space (40-59) ---
    /// Out of memory
    OutOfMemory = 40,
    /// Memory mapping failed
    MappingFailed = 41,
    /// Unmapping failed
    UnmappingFailed = 42,
    /// Invalid address (virt or phys)
    InvalidAddress = 43,

    // --- IPC (60-79) ---
    /// Invalid endpoint capability
    InvalidEndpoint = 60,
    /// IPC protocol mismatch
    InvalidProtocol = 61,
    /// Message too long / buffer overflow
    MessageTooLong = 62,
    /// Reply failed (no receiver)
    ReplyFailed = 63,

    // --- Resources & Lookup (80-99) ---
    /// Item not found
    NotFound = 80,
    /// Item already exists
    AlreadyExists = 81,
    /// Resource is busy/in-use
    ResourceBusy = 82,

    // --- Device & IO (100-119) ---
    /// Generic IO error
    IoError = 100,
    /// Hardware device error
    DeviceError = 101,
    /// Invalid configuration
    InvalidConfig = 102,

    #[num_enum(default)]
    Unknown = 255,
}

impl Into<usize> for Error {
    fn into(self) -> usize {
        self as usize
    }
}
