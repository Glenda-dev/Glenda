use crate::cap::Capability;
use crate::cpu;
use crate::hal;
use crate::ipc;
use crate::proc::scheduler;
use crate::proc::scheduler::DEFAULT_TIMESLICE;
use crate::sync::spinlock::SpinLock;

static ALARM_LOCK: SpinLock<Option<(usize, Capability)>> = SpinLock::new(None);

pub fn init() {
    let now = hal::timer::get_time();
    cpu::get().last_tick_time = now;
    program_next_tick();
}

pub fn program_next_tick() {
    let now = hal::timer::get_time();
    let cpu = cpu::get();
    let last = cpu.last_tick_time;
    let elapsed = if now > last && last > 0 { now - last } else { 0 };
    cpu.last_tick_time = now;

    // 减少当前线程的时间片
    if let Some(tcb_ptr) = scheduler::current() {
        let tcb = unsafe { &mut *tcb_ptr };
        if tcb.timeslice > elapsed {
            tcb.timeslice -= elapsed;
        } else {
            tcb.timeslice = 0;
        }
    }

    // 检查闹钟
    let alarm_due = {
        let mut alarm = ALARM_LOCK.lock();
        if let Some((time, _)) = alarm.as_ref() {
            if now >= *time { alarm.take() } else { None }
        } else {
            None
        }
    };

    if let Some((_, ntfn)) = alarm_due {
        if ntfn.cap_type() == crate::cap::CapType::Endpoint {
            let ep = unsafe { ntfn.obj_ptr().as_mut::<ipc::Endpoint>() };
            let _ = ipc::notify(ep, ntfn.get_badge());
        }
    }

    // Tickless: 计算下一个中断时间
    // 默认下一个时间片结束
    let tcb_slice = if let Some(tcb_ptr) = scheduler::current() {
        let tcb = unsafe { &*tcb_ptr };
        if tcb.timeslice > 0 { tcb.timeslice } else { DEFAULT_TIMESLICE }
    } else {
        DEFAULT_TIMESLICE
    };

    let mut next = now + tcb_slice;

    // 如果有闹钟，取最小者
    {
        let alarm = ALARM_LOCK.lock();
        if let Some((time, _)) = alarm.as_ref() {
            if *time > now && *time < next {
                next = *time;
            }
        }
    }

    hal::timer::set_next_event(next);
}

pub fn set_alarm(ms: usize, notification: Capability) {
    let now = hal::timer::get_time();
    let time = now + ms;
    {
        let mut alarm = ALARM_LOCK.lock();
        *alarm = Some((time, notification));
    }
    program_next_tick();
}
