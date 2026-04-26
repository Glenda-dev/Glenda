#![no_std]
#![no_main]

#[macro_use]
extern crate glenda;
extern crate alloc;

use glenda::cap::{MONITOR_CAP, CSPACE_CAP, Rights};
use glenda::client::{ResourceClient, NetworkClient};
use glenda::interface::{ResourceService, NetworkService, SocketService, CSpaceService};
use glenda::ipc::Badge;
use glenda::protocol::resource::{ResourceType, NET_ENDPOINT};
use glenda::protocol::network::{AF_INET, SOCK_RAW, IPPROTO_ICMP};
use glenda::utils::manager::CSpaceManager;
use smoltcp::phy::ChecksumCapabilities;
use smoltcp::wire::{Icmpv4Packet, Icmpv4Repr};

#[unsafe(no_mangle)]
fn main() -> usize {
    glenda::console::init_logging("ping-test");
    log!("Ping Test starting...");

    let mut res_client = ResourceClient::new(MONITOR_CAP);
    let mut cspace = CSpaceManager::new(CSPACE_CAP, 16);
    
    // 1. Get Gopher endpoint
    let net_ep_cap = res_client.get_cap(Badge::null(), ResourceType::Endpoint, NET_ENDPOINT, glenda::cap::ENDPOINT_SLOT)
        .expect("Failed to get net endpoint");

    let mut net_factory = NetworkClient::new(glenda::cap::Endpoint::from(net_ep_cap));

    // 2. Create ICMP socket
    log!("Creating ICMP socket...");
    let badge_bits = net_factory.socket(AF_INET, SOCK_RAW, IPPROTO_ICMP).expect("Failed to create socket");
    let badge = Badge::new(badge_bits);

    // 3. Mint badged endpoint for the socket
    let sock_slot = cspace.alloc(&mut res_client).expect("Failed to alloc slot");
    CSPACE_CAP.mint_self(net_ep_cap, sock_slot, badge, Rights::ALL).expect("Failed to mint socket cap");
    
    let mut sock_client = NetworkClient::new(glenda::cap::Endpoint::from(sock_slot));

    // 4. Connect to target (10.0.2.2 is usually QEMU gateway)
    let target_ip = [10, 0, 2, 2];
    log!("Connecting to 10.0.2.2...");
    sock_client.connect(&target_ip).expect("Failed to connect");

    // 5. Prepare ICMP Echo Request
    let mut icmp_buffer = [0u8; 8];
    let repr = Icmpv4Repr::EchoRequest {
        ident: 0x1234,
        seq_no: 1,
        data: &[],
    };
    let mut packet = Icmpv4Packet::new_unchecked(&mut icmp_buffer);
    repr.emit(&mut packet, &ChecksumCapabilities::default());
    packet.fill_checksum();

    // 6. Send Ping
    log!("Sending Ping...");
    sock_client.send(&icmp_buffer, 0).expect("Failed to send ping");

    // 7. Receive Reply (Blocking)
    log!("Waiting for reply...");
    let mut rx_buffer = [0u8; 2048];
    match sock_client.recv(&mut rx_buffer, 0) {
        Ok(len) => {
            let rx_packet = Icmpv4Packet::new_checked(&rx_buffer[..len]).expect("Invalid ICMP packet");
            match Icmpv4Repr::parse(&rx_packet, &ChecksumCapabilities::default()) {
                Ok(Icmpv4Repr::EchoReply { ident, seq_no, .. }) => {
                    log!("Received Echo Reply: ident={:#x}, seq_no={}", ident, seq_no);
                    if ident == 0x1234 && seq_no == 1 {
                        log!("Ping successful!");
                    }
                }
                Ok(other) => {
                    log!("Received other ICMP packet: {:?}", other);
                }
                Err(e) => {
                    log!("Failed to parse ICMP packet: {:?}", e);
                }
            }
        }
        Err(e) => {
            error!("Recv error: {:?}", e);
            return 1;
        }
    }

    log!("Ping Test finished.");
    0
}
