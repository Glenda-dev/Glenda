#![no_std]
#![no_main]

#[macro_use]
extern crate glenda;
extern crate alloc;

use glenda::cap::{
    CapPtr, CapType, ENDPOINT_CAP, ENDPOINT_SLOT, Endpoint, MONITOR_CAP, REPLY_CAP, REPLY_SLOT,
};
use glenda::client::{InitClient, ResourceClient};
use glenda::interface::{InitService, ResourceService};
use glenda::ipc::{Badge, MsgFlags, MsgTag, UTCB};
use glenda::protocol::resource::{INIT_ENDPOINT, ResourceType};

const IPC_TEST_SERVER_ID: usize = 0x1234;
const IPC_TEST_PROTO: usize = 0x55AA;
const INIT_SLOT: CapPtr = CapPtr::from(18);

#[unsafe(no_mangle)]
fn main() -> usize {
    glenda::console::init_logging("ipc-server");
    log!("IPC Performance Server starting...");

    let mut res_client = ResourceClient::new(MONITOR_CAP);

    // 1. Allocate an endpoint
    let ep_cap = res_client
        .alloc(Badge::null(), CapType::Endpoint, 0, ENDPOINT_SLOT)
        .expect("Failed to allocate endpoint");

    // 2. Register the endpoint with Warren so client can find it
    res_client
        .register_cap(Badge::null(), ResourceType::Endpoint, IPC_TEST_SERVER_ID, ep_cap)
        .expect("Failed to register endpoint");

    log!("Endpoint registered with ID {:#x}", IPC_TEST_SERVER_ID);

    res_client
        .get_cap(Badge::null(), ResourceType::Endpoint, INIT_ENDPOINT, INIT_SLOT)
        .expect("Failed to get endpoint capability");

    let mut init_client = InitClient::new(Endpoint::from(INIT_SLOT));

    let _ =
        init_client.report_service(Badge::null(), glenda::protocol::init::ServiceState::Running);
    // 4. Server loop
    let utcb = unsafe { UTCB::new() };
    loop {
        utcb.clear();
        utcb.set_reply_window(REPLY_SLOT);
        if let Err(e) = ENDPOINT_CAP.recv(utcb) {
            error!("Recv error: {:?}", e);
            continue;
        }

        let tag = utcb.get_msg_tag();
        if tag.proto() == IPC_TEST_PROTO {
            // Echo back the timestamp (MR0)
            let t = utcb.get_mr(0);
            let label = tag.label();
            let mrs_count = utcb.get_mrs_count();

            utcb.clear();
            utcb.set_mr(0, t);

            // If it was a "normal path" call (mrs_count > 4), we might want to reply with normal path too
            // but for simplicity, we just echo the same amount of MRs if we want to be fair.
            if mrs_count > 4 {
                for i in 1..mrs_count {
                    utcb.set_mr(i, 0);
                }
            }

            utcb.set_msg_tag(MsgTag::new(IPC_TEST_PROTO, label, MsgFlags::OK));

            if let Err(e) = REPLY_CAP.reply(utcb) {
                error!("Reply error: {:?}", e);
            }
        }
    }
}
