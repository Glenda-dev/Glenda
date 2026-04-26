#![no_std]
#![no_main]

#[macro_use]
extern crate glenda;
extern crate alloc;

use glenda::arch::time::get_time;
use glenda::cap::{ENDPOINT_SLOT, MONITOR_CAP};
use glenda::client::ResourceClient;
use glenda::interface::ResourceService;
use glenda::ipc::{Badge, MsgFlags, MsgTag, UTCB};
use glenda::protocol::resource::ResourceType;

const IPC_TEST_SERVER_ID: usize = 0x1234;
const IPC_TEST_PROTO: usize = 0x55AA;
const IPC_TEST_LABEL_FAST: usize = 0x01;
const IPC_TEST_LABEL_NORMAL: usize = 0x02;

#[unsafe(no_mangle)]
fn main() -> usize {
    glenda::console::init_logging("ipc-client");
    log!("IPC Performance Client starting...");

    let mut res_client = ResourceClient::new(MONITOR_CAP);

    // 1. Get the server endpoint
    let server_ep_cap = res_client
        .get_cap(Badge::null(), ResourceType::Endpoint, IPC_TEST_SERVER_ID, ENDPOINT_SLOT)
        .expect("Failed to get server endpoint");

    let ep = glenda::cap::Endpoint::from(server_ep_cap);
    let utcb = unsafe { UTCB::new() };

    log!("Starting IPC performance test...");

    // Warm up
    for _ in 0..10 {
        utcb.clear();
        utcb.set_mr(0, 0);
        utcb.set_msg_tag(MsgTag::new(IPC_TEST_PROTO, IPC_TEST_LABEL_FAST, MsgFlags::NONE));
        let _ = ep.call(utcb);
    }

    // Test Fast Path
    let mut total_fast = 0u64;
    let count = 100;
    for _ in 0..count {
        let t1 = get_time();
        utcb.clear();
        utcb.set_mr(0, t1 as usize);
        utcb.set_msg_tag(MsgTag::new(IPC_TEST_PROTO, IPC_TEST_LABEL_FAST, MsgFlags::NONE));

        let _ = ep.call(utcb);

        let t4 = get_time();
        total_fast += t4.wrapping_sub(t1);
    }
    log!("Fast path average: {} ticks", total_fast / count as u64);

    // Test Normal Path (Force HAS_MRS flag by using 5 MRs)
    let mut total_normal = 0u64;
    for _ in 0..count {
        let t1 = get_time();
        utcb.clear();
        utcb.set_mr(0, t1 as usize);
        utcb.set_mr(1, 0);
        utcb.set_mr(2, 0);
        utcb.set_mr(3, 0);
        utcb.set_mr(4, 0); // mrs_count = 5

        utcb.set_msg_tag(MsgTag::new(IPC_TEST_PROTO, IPC_TEST_LABEL_NORMAL, MsgFlags::NONE));

        let _ = ep.call(utcb);

        let t4 = get_time();
        total_normal += t4.wrapping_sub(t1);
    }
    log!("Normal path average: {} ticks", total_normal / count as u64);

    log!("IPC performance test finished.");
    0
}
