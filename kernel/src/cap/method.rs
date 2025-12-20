pub mod ipcmethod {
    pub const SEND: usize = 1;
    pub const RECV: usize = 2;
}

pub mod tcbmethod {
    pub const CONFIGURE: usize = 1;
    pub const SET_PRIORITY: usize = 2;
    pub const SET_REGISTERS: usize = 3;
    pub const SET_FAULT_HANDLER: usize = 4;
    pub const RESUME: usize = 5;
    pub const SUSPEND: usize = 6;
}

pub mod pagetablemethod {
    pub const MAP: usize = 1;
    pub const UNMAP: usize = 2;
}

pub mod cnodemethod {
    pub const MINT: usize = 1;
    pub const COPY: usize = 2;
    pub const DELETE: usize = 3;
    pub const REVOKE: usize = 4;
}

pub mod untypedmethod {
    pub const RETYPE: usize = 1;
}

pub mod irqmethod {
    pub const SET_NOTIFICATION: usize = 1;
    pub const ACK: usize = 2;
    pub const CLEAR_BINDING: usize = 3;
    pub const SET_PRIORITY: usize = 4;
}
