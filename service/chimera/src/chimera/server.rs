use glenda::cap::{CapPtr, Endpoint, Reply};
use glenda::client::{InitClient, ResourceClient};
use glenda::error::Error;
use glenda::interface::{FaultService, InitService, ResourceService, SystemService};
use glenda::ipc::{Badge, MsgTag, UTCB};
use glenda::protocol;
use glenda::protocol::resource::{CHIMERA_ENDPOINT, ResourceType};

pub struct ChimeraManager<'a> {
    pub res_client: &'a mut ResourceClient,
    pub init_client: &'a mut InitClient,
    pub endpoint: Endpoint,
    pub reply: Reply,
    pub recv: CapPtr,
    pub running: bool,
}

impl<'a> ChimeraManager<'a> {
    pub fn new(res_client: &'a mut ResourceClient, init_client: &'a mut InitClient) -> Self {
        Self {
            res_client,
            init_client,
            endpoint: Endpoint::from(CapPtr::null()),
            reply: Reply::from(CapPtr::null()),
            recv: CapPtr::null(),
            running: false,
        }
    }
}

impl<'a> SystemService for ChimeraManager<'a> {
    fn init(&mut self) -> Result<(), Error> {
        self.init_client.report_service(Badge::null(), protocol::init::ServiceState::Starting)?;
        Ok(())
    }

    fn listen(&mut self, ep: Endpoint, reply: CapPtr, recv: CapPtr) -> Result<(), Error> {
        self.endpoint = ep;
        self.reply = Reply::from(reply);
        self.recv = recv;
        self.res_client.register_cap(
            Badge::null(),
            ResourceType::Endpoint,
            CHIMERA_ENDPOINT,
            ep.cap(),
        )?;
        Ok(())
    }

    fn run(&mut self) -> Result<(), Error> {
        if self.endpoint.cap().is_null() || self.reply.cap().is_null() || self.recv.is_null() {
            return Err(Error::NotInitialized);
        }
        self.init_client.report_service(Badge::null(), protocol::init::ServiceState::Running)?;
        self.running = true;
        while self.running {
            let mut utcb = unsafe { UTCB::new() };
            utcb.clear();
            utcb.set_reply_window(self.reply.cap());
            utcb.set_recv_window(self.recv);
            match self.endpoint.recv(&mut utcb) {
                Ok(_) => {}
                Err(e) => {
                    error!("Chimera recv error: {:?}", e);
                    continue;
                }
            }

            match self.dispatch(&mut utcb) {
                Ok(()) => {}
                Err(e) => {
                    if e == Error::Success {
                        continue;
                    }
                    error!("Chimera dispatch error: {:?}", e);
                    utcb.set_msg_tag(MsgTag::err());
                    utcb.set_mr(0, e as usize);
                }
            }

            if let Err(e) = self.reply(&mut utcb) {
                if e != Error::InvalidCapability {
                    error!("Chimera reply failed: {:?}", e);
                }
            }
        }
        Ok(())
    }

    fn dispatch(&mut self, utcb: &mut UTCB) -> Result<(), Error> {
        let badge = utcb.get_badge();
        let mrs = utcb.get_mrs();

        glenda::ipc_dispatch! {
            self, utcb,
            (protocol::KERNEL_PROTO, protocol::kernel::VIRT_EXIT) => |s: &mut Self, _u: &mut UTCB| {
                s.virt_exit(badge, mrs[0], mrs[1], mrs[2], mrs[3])
            },
            (protocol::KERNEL_PROTO, protocol::kernel::PAGE_FAULT) => |s: &mut Self, _u: &mut UTCB| {
                s.page_fault(badge, mrs[0], mrs[1], mrs[2])
            },
            (protocol::KERNEL_PROTO, protocol::kernel::ILLEGAL_INSTRUCTION) => |s: &mut Self, _u: &mut UTCB| {
                s.illegal_instruction(badge, mrs[0], mrs[1])
            },
            (protocol::KERNEL_PROTO, protocol::kernel::BREAKPOINT) => |s: &mut Self, _u: &mut UTCB| {
                s.breakpoint(badge, mrs[0])
            },
            (protocol::KERNEL_PROTO, protocol::kernel::ACCESS_FAULT) => |s: &mut Self, _u: &mut UTCB| {
                s.access_fault(badge, mrs[0], mrs[1])
            },
            (protocol::KERNEL_PROTO, protocol::kernel::ACCESS_MISALIGNED) => |s: &mut Self, _u: &mut UTCB| {
                s.access_misaligned(badge, mrs[0], mrs[1])
            },
            (protocol::KERNEL_PROTO, protocol::kernel::SYSCALL) => |s: &mut Self, _u: &mut UTCB| {
                s.handle_syscall(badge.bits(), mrs)
            },
            (protocol::KERNEL_PROTO, protocol::kernel::UNKNOWN_FAULT) => |s: &mut Self, _u: &mut UTCB| {
                s.unknown_fault(badge, mrs[0], mrs[1], mrs[2])
            },
            (_, _) => |_, _| Err(Error::InvalidProtocol)
        }
    }

    fn reply(&mut self, utcb: &mut UTCB) -> Result<(), Error> {
        self.reply.reply(utcb)
    }

    fn stop(&mut self) {
        self.running = false;
        let _ = self.init_client.report_service(Badge::null(), protocol::init::ServiceState::Stopped);
    }
}
