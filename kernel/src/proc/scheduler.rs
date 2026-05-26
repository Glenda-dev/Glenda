use super::thread::{ALL_THREADS, ALL_THREADS_LOCK};
use super::thread::{TCB, ThreadState};
use crate::boot;
use crate::cpu;
use crate::hal;
use crate::hal::cpu::MAX_CPUS;
use core::sync::atomic::{AtomicUsize, Ordering};

pub const DEFAULT_TIMESLICE_MS: usize = 100;
pub const MAX_PRIORITY: usize = 256;

#[inline]
pub fn get_default_timeslice() -> usize {
    hal::timer::get_freq() / (1000 / DEFAULT_TIMESLICE_MS)
}

#[derive(Debug)]
pub struct TcbQueue {
    head: Option<*mut TCB>,
    tail: Option<*mut TCB>,
}

unsafe impl Send for TcbQueue {}

impl TcbQueue {
    pub const fn new() -> Self {
        Self { head: None, tail: None }
    }

    pub fn push_back(&mut self, tcb: *mut TCB) {
        unsafe {
            (*tcb).prev = self.tail;
            (*tcb).next = None;
            if let Some(tail) = self.tail {
                (*tail).next = Some(tcb);
            } else {
                self.head = Some(tcb);
            }
            self.tail = Some(tcb);
        }
    }

    pub fn pop_front(&mut self) -> Option<*mut TCB> {
        if let Some(head) = self.head {
            unsafe {
                let next = (*head).next;
                if let Some(next_ptr) = next {
                    (*next_ptr).prev = None;
                } else {
                    self.tail = None;
                }
                self.head = next;
                (*head).next = None;
                (*head).prev = None;
            }
            Some(head)
        } else {
            None
        }
    }

    pub fn remove(&mut self, tcb_ptr: *mut TCB) {
        unsafe {
            let tcb = &mut *tcb_ptr;

            // Check prev
            if let Some(prev) = tcb.prev {
                (*prev).next = tcb.next;
            } else {
                // Is head
                if self.head == Some(tcb_ptr) {
                    self.head = tcb.next;
                }
            }

            // Check next
            if let Some(next) = tcb.next {
                (*next).prev = tcb.prev;
            } else {
                // Is tail
                if self.tail == Some(tcb_ptr) {
                    self.tail = tcb.prev;
                }
            }

            tcb.next = None;
            tcb.prev = None;
        }
    }
}

static CURRENT_TCB: [AtomicUsize; MAX_CPUS] = [const { AtomicUsize::new(0) }; MAX_CPUS];
static LAST_WATCHDOG_DUMP: AtomicUsize = AtomicUsize::new(0);
static LAST_WATCHDOG_PROGRESS: AtomicUsize = AtomicUsize::new(0);
static WATCHDOG_EVENT_COUNT: AtomicUsize = AtomicUsize::new(0);

const WATCHDOG_FIRST_DUMP_SECS: usize = 8;
const WATCHDOG_INTERVAL_SECS: usize = 15;

macro_rules! watchdog_event {
    ($($arg:tt)*) => {{
        if WATCHDOG_EVENT_COUNT.fetch_add(1, Ordering::Relaxed) < 64 {
            debug!($($arg)*);
        }
    }};
}

fn mark_watchdog_progress() {
    LAST_WATCHDOG_PROGRESS.store(hal::timer::get_time(), Ordering::Relaxed);
}

/// 将线程加入调度队列
pub fn add_thread(tcb: &mut TCB) {
    add_thread_inner(tcb, true);
}

fn add_thread_inner(tcb: &mut TCB, send_ipi: bool) {
    let current_hart_id = hal::cpu::cpu_id();
    let ptr = tcb as *mut TCB;
    let _tcb_guard = tcb.lock.lock();

    // 根据 affinity 决定目标核心
    // 假设 TCB 中包含 affinity 字段。如果 affinity >= MAX_CPUS，则表示不绑定，默认使用当前核心
    let target_hart_id = select_target_cpu(tcb, current_hart_id);

    tcb.cpu_id = target_hart_id;

    // 获取目标 CPU 的运行队列
    // 注意：访问全局 HARTS 数组需要 unsafe，且要小心死锁（这里只持有一个锁，是安全的）
    let target_hart = unsafe { &cpu::CPUS[target_hart_id] };
    let mut queues = target_hart.ready_queues.lock();
    let prio = tcb.priority as usize;

    // 确保状态正确
    if tcb.state == ThreadState::Ready {
        queues[prio].push_back(ptr);
        mark_watchdog_progress();
    }

    drop(queues);
    // 如果目标核心不是当前核心，发送 IPI 唤醒它
    // 这样目标核心如果处于 WFI 状态会被唤醒，或者在运行其他线程时触发调度检查
    if send_ipi && target_hart_id != current_hart_id {
        let mask = 1 << target_hart_id;
        let _ = hal::irq::send_ipi(mask, 0);
    }
}

pub fn remove_thread(tcb: &mut TCB) {
    // 只有 Ready 或 Running 状态的线程才在调度器管辖范围内
    // Blocked/Queue 状态在其他结构中 (如 Endpoint)

    // Ensure atomic operation regarding interrupts
    // Use push_off to nest correctly with SpinLocks
    cpu::push_off();
    let ptr = tcb as *mut TCB;
    let _tcb_guard = tcb.lock.lock();

    match tcb.state {
        ThreadState::Running => {
            // 如果是 Running 状态，我们需要根据是否是当前 CPU 来处理
            let current_hart_id = hal::cpu::cpu_id();
            if tcb.cpu_id == current_hart_id {
                // 如果是当前 CPU 正在运行的线程（也就是自己），将其设为 Inactive
                // 下一次 yield 或 schedule 时会切换走
                // 调用者可能是此线程（自杀）或另一线程（monitor 杀子线程）。
                // 为避免并发状态不一致，先将其标记为 Inactive。
                // TCB 结构的最终生命周期由能力清理路径负责。
                // 为了安全，设为 Inactive 是必须的。
                tcb.state = ThreadState::Inactive;
            } else {
                // 运行在其他 CPU
                // 需要发送 IPI 强制重新调度 / 停止该线程
                // 简单实现：设为 Inactive，并发送 IPI
                tcb.state = ThreadState::Inactive;
                let mask = 1 << tcb.cpu_id;
                let _ = hal::irq::send_ipi(mask, 0);
            }
        }
        ThreadState::Ready => {
            // 在 Ready 队列中，移除它
            // 使用 tcb.cpu_id 找到对应队列
            let cpu_id = tcb.cpu_id;
            // Bound check for safety
            if cpu_id < MAX_CPUS {
                let cpu = unsafe { &cpu::CPUS[cpu_id] };
                let mut queues = cpu.ready_queues.lock();
                let prio = tcb.priority as usize;

                // 确保 tcb 在队列中？
                // TcbQueue::remove 会根据 pointers 移除。如果 pointers 乱了会很危险。
                // 假设状态一致性由锁保证。
                if prio < MAX_PRIORITY {
                    queues[prio].remove(ptr);
                }
                drop(queues);
            }
            tcb.state = ThreadState::Inactive;
        }
        _ => {
            // 其他状态下（如 BlockedRecv），它不在 Ready 队列，而在 Endpoint 等待队列。
            // Endpoint 的 cancel_badged_sends 等方法负责移除。
            // 这里至少保证状态为 Inactive。
            tcb.state = ThreadState::Inactive;
        }
    }

    cpu::pop_off();
}

/// 核心调度循环
/// 永远不会返回
pub fn scheduler() -> ! {
    loop {
        // 1. 关闭中断以保护调度逻辑
        // 在 RISC-V 中，这通常在进入异常处理时自动完成，但在 idle loop 中需要手动管理
        unsafe { hal::irq::disable() };

        let mut next_thread: Option<*mut TCB> = None;

        // 2. 寻找最高优先级的 Ready 线程
        {
            let cpu = cpu::get();
            // 从最高优先级 (255) 向下遍历
            'outer: for prio in (0..MAX_PRIORITY).rev() {
                loop {
                    let mut queues = cpu.ready_queues.lock();
                    if let Some(tcb_ptr) = queues[prio].pop_front() {
                        drop(queues); // 释放队列锁，避免与 TCB 锁产生 ABBA 死锁

                        let tcb = unsafe { &mut *tcb_ptr };
                        let _guard = tcb.lock.lock();
                        if tcb.state != ThreadState::Ready {
                            // 队列中扫描到 inactive 线程就直接移除
                            // 已经在 pop_front 中移除了
                            warn!(
                                "scheduler: Skipping thread {:p} with state {:?}",
                                tcb_ptr, tcb.state
                            );
                            continue; // 继续 loop 查找当前优先级下一个线程
                        }
                        log!(
                            "scheduler: Selected thread {:p} with priority {} for running",
                            tcb_ptr,
                            prio
                        );
                        next_thread = Some(tcb_ptr);
                        break 'outer;
                    } else {
                        drop(queues);
                        break; // 此优先级队列为空，尝试下一个优先级
                    }
                }
            }
        }

        // 3. Work Stealing: 如果本地队列为空，尝试从其他 CPU 偷取
        if next_thread.is_none() {
            for i in 0..MAX_CPUS {
                if i == hal::cpu::cpu_id() {
                    continue;
                }
                let target_cpu = unsafe { &cpu::CPUS[i] };
                if let Some(mut queues) = target_cpu.ready_queues.try_lock() {
                    for prio in (0..MAX_PRIORITY).rev() {
                        if let Some(tcb_ptr) = queues[prio].pop_front() {
                            let tcb = unsafe { &mut *tcb_ptr };
                            // 尝试锁定 TCB，避免死锁
                            if let Some(_guard) = tcb.lock.try_lock() {
                                if tcb.state == ThreadState::Ready {
                                    log!("scheduler: Stole thread {:p} from CPU {}", tcb_ptr, i);
                                    tcb.cpu_id = hal::cpu::cpu_id();
                                    next_thread = Some(tcb_ptr);
                                    break;
                                }
                            } else {
                                // 如果锁不住，放回对方队列前面？或者就由它去
                                queues[prio].push_back(tcb_ptr);
                            }
                        }
                    }
                }
                if next_thread.is_some() {
                    break;
                }
            }
        }

        if let Some(tcb_ptr) = next_thread {
            mark_watchdog_progress();
            // Lock TCB to ensure atomic transition to Running
            unsafe {
                let _tcb_guard = (*tcb_ptr).lock.lock();

                // 如果在此期间线程状态变了（极罕见），则不运行并重新开始调度
                if (*tcb_ptr).state != ThreadState::Ready {
                    warn!("scheduler: Thread {:p} state changed while selecting!", tcb_ptr);
                    continue;
                }

                // 如果时间片用完了，重新分配
                if (*tcb_ptr).timeslice == 0 {
                    (*tcb_ptr).timeslice = if (*tcb_ptr).timeslice_limit > 0 {
                        (*tcb_ptr).timeslice_limit
                    } else {
                        get_default_timeslice()
                    };
                }

                // Update kernel_hartid in TrapFrame to ensure correct CPU ID upon trap/syscall
                // This is critical for SMP!
                let cpuid = cpu::get().id;
                (*tcb_ptr).get_tf().set_cpuid(cpuid);

                // 更新状态
                (*tcb_ptr).state = ThreadState::Running;
            } // _tcb_guard drops here

            let tcb = unsafe { &mut *tcb_ptr };

            // 获取当前 CPU 的 CPU 结构
            let cpu = cpu::get();
            // 设置当前运行的线程
            set_current(tcb_ptr);
            // 执行上下文切换：从当前 CPU 的 idle context 切换到线程 context
            unsafe {
                hal::proc::switch_context(&mut cpu.context, &mut tcb.context);
            }
            set_current(core::ptr::null_mut());
            finish_pending_wakeup(tcb_ptr);

            // --- 线程返回 ---
            // 当线程被抢占或主动 yield 后，会回到这里
        } else {
            // 没有可运行的线程，进入低功耗等待
            log!("scheduler: No ready threads found, entering idle state");
            watchdog_idle();
            unsafe {
                hal::irq::wfi();
                hal::irq::enable();
            }
        }
    }
}

/// 主动放弃 CPU (Yield)
/// 将当前线程放回 Ready 队列末尾，并触发调度
pub fn yield_proc() {
    let tcb_ptr = match current() {
        Some(ptr) => ptr,
        None => return,
    };
    let mut context = cpu::get().context;

    let tcb = unsafe { &mut *tcb_ptr };

    // 锁定以防状态变迁不一致
    let _tcb_guard = tcb.lock.lock();
    // 只有 Running 状态的线程才能 yield
    if tcb.state == ThreadState::Running {
        tcb.state = ThreadState::Ready;
        drop(_tcb_guard); // add_thread will re-lock
        add_thread(tcb); // 放回队列末尾
    } else {
        drop(_tcb_guard);
    }

    // 切换回调度器 (context)
    unsafe {
        hal::proc::switch_context(&mut tcb.context, &mut context);
    }
}

/// 阻塞当前线程
/// 线程状态必须在调用此函数前被设置为 BlockedSend / BlockedRecv / Inactive
pub fn block_current_thread() {
    let tcb_ptr = match current() {
        Some(ptr) => ptr,
        None => return,
    };
    let mut context = cpu::get().context;

    let tcb = unsafe { &mut *tcb_ptr };

    // 锁定并断言
    let _tcb_guard = tcb.lock.lock();
    // 确保线程不再是 Running 状态
    assert!(
        tcb.state != ThreadState::Running,
        "Thread must set block state before calling block()"
    );
    drop(_tcb_guard);

    // 直接切换回调度器，不加入 Ready 队列
    unsafe {
        hal::proc::switch_context(&mut tcb.context, &mut context);
    }
}

/// 唤醒指定线程
/// 将线程状态设置为 Ready 并加入调度队列
pub fn wake_up(tcb: &mut TCB) {
    wake_up_inner(tcb, true, true);
}

/// 唤醒线程但不立即抢占当前线程；若目标在远端 CPU，仍发送 IPI。
///
/// 用于 IPC Call 的握手路径：caller 已经被标记为 BlockedCall，
/// 但在真正切回调度器前仍在当前 CPU 上执行。若此时让 receiver
/// 在其他 CPU 立即运行并 reply，caller 可能被重新入队并与当前栈并发运行。
pub fn wake_up_deferred(tcb: &mut TCB) {
    wake_up_inner(tcb, false, true);
}

fn wake_up_inner(tcb: &mut TCB, preempt: bool, send_ipi: bool) {
    let ptr = tcb as *mut TCB;
    let _tcb_guard = tcb.lock.lock();
    // 只有处于阻塞状态的线程才能被唤醒。
    // 如果是 Suspended 状态，它保持 Suspended，直到被 Resume 系统调用显式恢复。
    if tcb.state == ThreadState::BlockedSend
        || tcb.state == ThreadState::BlockedRecv
        || tcb.state == ThreadState::BlockedCall
    {
        if is_current_on_any_cpu(ptr) {
            tcb.wake_pending = true;
            watchdog_event!(
                "watchdog: deferred wake target={:p} state={:?} cpu={} caller_cpu={}",
                ptr,
                tcb.state,
                tcb.cpu_id,
                hal::cpu::cpu_id()
            );
            drop(_tcb_guard);
            return;
        }
        tcb.state = ThreadState::Ready;
        drop(_tcb_guard);
        add_thread_inner(tcb, send_ipi);

        // 如果被唤醒线程优先级高于当前线程，触发抢占 (reschedule)
        let current_hart_id = hal::cpu::cpu_id();
        let target_hart_id = select_target_cpu(tcb, current_hart_id);

        if preempt && target_hart_id == current_hart_id {
            if let Some(curr_ptr) = current() {
                let curr = unsafe { &*curr_ptr };
                if tcb.priority >= curr.priority {
                    reschedule();
                }
            }
        }
    }
}

fn finish_pending_wakeup(tcb_ptr: *mut TCB) {
    let tcb = unsafe { &mut *tcb_ptr };
    let _guard = tcb.lock.lock();
    if tcb.wake_pending {
        tcb.wake_pending = false;
        tcb.state = ThreadState::Ready;
        watchdog_event!(
            "watchdog: finish pending wake target={:p} cpu={}",
            tcb_ptr,
            hal::cpu::cpu_id()
        );
        drop(_guard);
        add_thread(tcb);
    }
}

fn is_current_on_any_cpu(tcb_ptr: *mut TCB) -> bool {
    let encoded = tcb_ptr as usize;
    for cpu in 0..MAX_CPUS {
        if CURRENT_TCB[cpu].load(Ordering::Acquire) == encoded {
            return true;
        }
    }
    false
}

fn select_target_cpu(tcb: &TCB, fallback_cpu: usize) -> usize {
    if tcb.affinity < MAX_CPUS {
        return tcb.affinity;
    }

    let cpus = boot::get_cpu_count().min(MAX_CPUS);
    if tcb.cpu_id < cpus {
        let cpu = unsafe { &cpu::CPUS[tcb.cpu_id] };
        if cpu.enabled {
            return tcb.cpu_id;
        }
    }

    fallback_cpu
}

/// 触发重新调度
/// 抢占当前线程，进入调度器
/// 通常在修改线程优先级后调用
pub fn reschedule() {
    let tcb_ptr = match current() {
        Some(ptr) => ptr,
        None => return,
    };
    let mut context = cpu::get().context;
    let tcb = unsafe { &mut *tcb_ptr };
    // 将当前线程状态设置为 Ready 并加入队列
    let should_enqueue = {
        let _guard = tcb.lock.lock();
        if tcb.state == ThreadState::Running {
            tcb.state = ThreadState::Ready;
            true
        } else {
            false
        }
    };
    if should_enqueue {
        add_thread(tcb);
    }
    // 切换回调度器
    unsafe {
        hal::proc::switch_context(&mut tcb.context, &mut context);
    }
}

pub fn current() -> Option<*mut TCB> {
    let cpu = hal::cpu::cpu_id();
    let tcb_ptr = CURRENT_TCB[cpu].load(Ordering::Acquire) as *mut TCB;
    if tcb_ptr.is_null() { None } else { Some(tcb_ptr) }
}

pub fn set_current(tcb_ptr: *mut TCB) {
    set_current_ptr(tcb_ptr);
}

pub fn set_current_ptr(tcb_ptr: *mut TCB) {
    let cpu = hal::cpu::cpu_id();
    CURRENT_TCB[cpu].store(tcb_ptr as usize, Ordering::Release);
}

pub fn get_cpu_context() -> *mut hal::proc::ProcContext {
    &mut crate::cpu::get().context as *mut _
}

pub fn watchdog_tick(now: usize) {
    if hal::cpu::cpu_id() != 0 {
        return;
    }

    let freq = hal::timer::get_freq();
    if freq == 0 {
        return;
    }

    let progress = LAST_WATCHDOG_PROGRESS.load(Ordering::Relaxed);
    if progress == 0 {
        LAST_WATCHDOG_PROGRESS.store(now, Ordering::Relaxed);
        return;
    }

    let stalled_for = now.saturating_sub(progress);
    if stalled_for < freq.saturating_mul(WATCHDOG_FIRST_DUMP_SECS) {
        return;
    }

    let interval = freq.saturating_mul(WATCHDOG_INTERVAL_SECS);
    let last = LAST_WATCHDOG_DUMP.load(Ordering::Relaxed);
    if now.saturating_sub(last) < interval {
        return;
    }

    if LAST_WATCHDOG_DUMP.compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed).is_ok()
    {
        dump_watchdog(now);
    }
}

pub fn watchdog_idle() {
    watchdog_tick(hal::timer::get_time());
}

fn dump_watchdog(now: usize) {
    let progress = LAST_WATCHDOG_PROGRESS.load(Ordering::Relaxed);
    debug!(
        "watchdog: now={} freq={} cpus={} cpu0_noff={} stalled_ticks={}",
        now,
        hal::timer::get_freq(),
        boot::get_cpu_count(),
        cpu::get().noff,
        now.saturating_sub(progress)
    );

    for id in 0..boot::get_cpu_count().min(MAX_CPUS) {
        let curr = CURRENT_TCB[id].load(Ordering::Acquire) as *mut TCB;
        let cpu_ref = unsafe { &cpu::CPUS[id] };
        let mut ready_total = 0usize;
        let mut top_prio = 0usize;
        let mut top_count = 0usize;
        let ready_locked = if let Some(queues) = cpu_ref.ready_queues.try_lock() {
            for prio in 0..MAX_PRIORITY {
                let mut curr = queues[prio].head;
                let mut count = 0usize;
                while let Some(ptr) = curr {
                    count += 1;
                    ready_total += 1;
                    curr = unsafe { (*ptr).next };
                    if count > 4096 {
                        debug!("watchdog: cpu={} ready queue prio={} appears cyclic", id, prio);
                        break;
                    }
                }
                if count > 0 {
                    top_prio = prio;
                    top_count = count;
                }
            }
            false
        } else {
            true
        };
        debug!(
            "watchdog: cpu={} enabled={} noff={} intena={} current={:p} ready_total={} top_prio={} top_count={} ready_locked={}",
            id,
            cpu_ref.enabled,
            cpu_ref.noff,
            cpu_ref.intena,
            curr,
            ready_total,
            top_prio,
            top_count,
            ready_locked,
        );
    }

    unsafe {
        let _all_threads_guard = ALL_THREADS_LOCK.lock();
        let mut curr = ALL_THREADS;
        let mut count = 0usize;
        while let Some(ptr) = curr {
            let tcb = &*ptr;
            if let Some(_guard) = tcb.lock.try_lock() {
                debug!(
                    "watchdog: tcb={:p} state={:?} prio={}/{} cpu={} aff={} timeslice={} wake_pending={} prev={:p} next={:p} partner={:p} active_ep={:#x} caller={:p} badge={:?} cap={}",
                    ptr,
                    tcb.state,
                    tcb.priority,
                    tcb.base_priority,
                    tcb.cpu_id,
                    tcb.affinity,
                    tcb.timeslice,
                    tcb.wake_pending,
                    tcb.prev.unwrap_or(core::ptr::null_mut()),
                    tcb.next.unwrap_or(core::ptr::null_mut()),
                    tcb.ipc_partner.unwrap_or(core::ptr::null_mut()),
                    tcb.ipc_active_ep.unwrap_or(0),
                    tcb.ipc_caller.unwrap_or(core::ptr::null_mut()),
                    tcb.ipc_badge,
                    tcb.ipc_cap.is_some(),
                );
                curr = tcb.global_next;
            } else {
                debug!("watchdog: tcb={:p} locked", ptr);
                curr = (*ptr).global_next;
            }

            count += 1;
            if count > 128 {
                debug!("watchdog: aborting thread dump after {} entries", count);
                break;
            }
        }
    }
}

pub fn ps() {
    printk!("ID (Addr)  Prio  State       Aff\n");
    printk!("--------------------------------\n");

    unsafe {
        let _all_threads_guard = ALL_THREADS_LOCK.lock();
        let mut curr = ALL_THREADS;
        while let Some(ptr) = curr {
            let tcb = &*ptr;

            let state_str = match tcb.state {
                ThreadState::Inactive => "Inactive   ",
                ThreadState::Suspended => "Suspended  ",
                ThreadState::Ready => "Ready      ",
                ThreadState::Running => "Running    ",
                ThreadState::BlockedSend => "BlockedSend",
                ThreadState::BlockedRecv => "BlockedRecv",
                ThreadState::BlockedCall => "BlockedCall",
            };

            // 使用地址作为 ID
            let aff = if tcb.affinity == usize::MAX { -1 } else { tcb.affinity as isize };

            printk!("{:#08x} {:<5} {:<11} {}\n", ptr as usize, tcb.priority, state_str, aff);

            curr = tcb.global_next;
        }
    }
}
