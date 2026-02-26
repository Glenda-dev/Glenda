use crate::cap::Capability;
use crate::cpu;
use crate::hal;
use crate::ipc;
use crate::proc::scheduler;
use crate::proc::scheduler::get_default_timeslice;
use crate::sync::spinlock::SpinLock;

pub struct Alarm {
    pub time: usize,
    pub ep: Capability,
}

static ALARM_LOCK: SpinLock<Option<Alarm>> = SpinLock::new(None);

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
        if let Some(alarm_data) = alarm.as_ref() {
            let time = alarm_data.time;
            if now >= time { alarm.take() } else { None }
        } else {
            None
        }
    };

    if let Some(alarm_data) = alarm_due {
        let ntfn = alarm_data.ep;
        if ntfn.cap_type() == crate::cap::CapType::Endpoint {
            let ep = unsafe { ntfn.obj_ptr().as_mut::<ipc::Endpoint>() };
            let _ = ipc::notify(ep, ntfn.get_badge());
        }
    }

    // Tickless: 计算下一个中断时间
    // 默认下一个时间片结束
    let tcb_slice = if let Some(tcb_ptr) = scheduler::current() {
        let tcb = unsafe { &*tcb_ptr };
        if tcb.timeslice > 0 { tcb.timeslice } else { get_default_timeslice() }
    } else {
        get_default_timeslice()
    };

    let mut next = now + tcb_slice;

    // 如果有闹钟，取最小者
    {
        let alarm = ALARM_LOCK.lock();
        if let Some(alarm_data) = alarm.as_ref() {
            let time = alarm_data.time;
            if time > now && time < next {
                next = time;
            }
        }
    }

    hal::timer::set_next_event(next);
}

pub fn set_alarm(time: usize, ep: Capability) {
    {
        let mut alarm = ALARM_LOCK.lock();
        *alarm = Some(Alarm { time, ep });
    }
    program_next_tick();
}
