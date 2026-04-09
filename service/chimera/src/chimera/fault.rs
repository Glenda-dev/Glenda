use super::ChimeraManager;
use glenda::error::Error;
use glenda::interface::FaultService;
use glenda::ipc::{Badge, MsgArgs};

impl<'a> FaultService for ChimeraManager<'a> {
    fn page_fault(
        &mut self,
        badge: Badge,
        addr: usize,
        pc: usize,
        cause: usize,
    ) -> Result<(), Error> {
        warn!(
            "Chimera guest page fault: badge={:#x} addr={:#x} pc={:#x} cause={:#x}",
            badge.bits(),
            addr,
            pc,
            cause
        );
        Ok(())
    }

    fn unknown_fault(
        &mut self,
        badge: Badge,
        cause: usize,
        value: usize,
        pc: usize,
    ) -> Result<(), Error> {
        warn!(
            "Chimera unknown fault: badge={:#x} cause={:#x} value={:#x} pc={:#x}",
            badge.bits(),
            cause,
            value,
            pc
        );
        Ok(())
    }

    fn illegal_instruction(&mut self, badge: Badge, inst: usize, pc: usize) -> Result<(), Error> {
        warn!(
            "Chimera illegal instruction: badge={:#x} inst={:#x} pc={:#x}",
            badge.bits(),
            inst,
            pc
        );
        Ok(())
    }

    fn breakpoint(&mut self, badge: Badge, pc: usize) -> Result<(), Error> {
        warn!("Chimera breakpoint: badge={:#x} pc={:#x}", badge.bits(), pc);
        Ok(())
    }

    fn access_fault(&mut self, badge: Badge, addr: usize, pc: usize) -> Result<(), Error> {
        warn!(
            "Chimera access fault: badge={:#x} addr={:#x} pc={:#x}",
            badge.bits(),
            addr,
            pc
        );
        Ok(())
    }

    fn access_misaligned(&mut self, badge: Badge, addr: usize, pc: usize) -> Result<(), Error> {
        warn!(
            "Chimera access misaligned: badge={:#x} addr={:#x} pc={:#x}",
            badge.bits(),
            addr,
            pc
        );
        Ok(())
    }

    fn virt_exit(
        &mut self,
        badge: Badge,
        reason: usize,
        detail0: usize,
        detail1: usize,
        detail2: usize,
    ) -> Result<(), Error> {
        log!(
            "Chimera virt exit: badge={:#x} reason={:#x} d0={:#x} d1={:#x} d2={:#x}",
            badge.bits(),
            reason,
            detail0,
            detail1,
            detail2
        );
        Ok(())
    }

    fn handle_syscall(&mut self, badge: usize, args: MsgArgs) -> Result<(), Error> {
        warn!(
            "Chimera non-native syscall: badge={:#x} args=[{:#x},{:#x},{:#x},{:#x},{:#x},{:#x},{:#x},{:#x}]",
            badge,
            args[0],
            args[1],
            args[2],
            args[3],
            args[4],
            args[5],
            args[6],
            args[7]
        );
        Ok(())
    }
}
