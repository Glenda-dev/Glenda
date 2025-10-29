# Lab-3 Report

## 实现目标

- 建立 S 模式陷阱入口与上下文保存/恢复
- 接入 PLIC：初始化全局与 per-hart 使能，识别 UART 外设中断
- 完善 UART 中断回显：支持换行与 Backspace
- 实现时钟中断：基于 SBI 的 STIP 周期滴答与全局 ticks 计数
- 预置 M 模式定时器向量路径（未默认启用，便于对照/扩展）
- 编写滴答与 UART 测试，用屏幕输出观察行为

## 具体步骤

### 1. 搭建 S 模式陷阱框架（入口、上下文、分发）

- 入口与保存/恢复
  - 在 `kernel/src/trap/vector.S` 定义 `kernel_vector`：按 32 个通用寄存器顺序入栈，调用 Rust 侧处理函数后恢复并 `sret` 返回。
  - 在 `kernel/src/trap/vector.rs::set_vector` 将 `stvec` 设为 `Direct` 模式，入口为 `kernel_vector`。
- 上下文结构
  - 在 `kernel/src/trap/context.rs` 定义 `TrapContext`，与 `kernel_vector` 栈布局一致，便于在 Rust 中访问/调试。
- 处理与分发
  - 在 `kernel/src/trap/handler.rs` 实现 `trap_kernel_handler`：读取 `scause/sepc/stval/sstatus`，按 `Exception` 与 `Interrupt` 分类分发。
  - `exception_handler`：打印详细信息并 `panic`（不期望的异常尽早失败，便于定位）。
  - `interrupt_handler`：主要处理三类 S 模式中断：
    - 外部中断（code=9）-> `external_interrupt_handler`
    - 时钟中断（STIP，code=5）-> `timer_interrupt_handler_stip`
    - 软件中断（SSIP，code=1）-> `timer_interrupt_handler_ssip`
  - 细节：在 `inittraps_hart` 时将 `sscratch` 写入当前 `hartid`，供中断处理快速获取当前核。

### 2. PLIC 初始化与地址映射g

- PLIC 全局与 per-hart 初始化（`kernel/src/trap/plic.rs`）
  - `init`：设置 `UART_IRQ`（10）优先级为 1。
  - `init_hart`：开启 S 上下文的 UART 使能位，阈值设为 0（允许所有优先级>0 的中断）。
  - `claim/complete`：按 RISC-V PLIC 规范从 S 上下文通道读取/写回。
- 地址获取与映射
  - PLIC 基址从 DTB 解析（`kernel/src/dtb.rs::plic_base`）。
  - 在页表初始化阶段映射 PLIC 寄存器与每核上下文窗口（`kernel/src/mem/vm.rs` 中对低区寄存器和 `0x200000 + ctx*0x1000` 的 S 上下文页进行映射），确保 S 模式可访问。

### 3. UART 中断接入与回显

- 打开 UART 接收中断
  - 在 `trap::inittraps` 中，依据 DTB 解析到的 16550 兼容 UART 配置，计算寄存器步长并对 IER（index=1）写 `0x01` 以使能 RX 中断。
- 外部中断分发与回显处理
  - `external_interrupt_handler`：从 PLIC 读取中断源 ID；若为 `UART_IRQ` 则调用 `uart_interrupt_handler`，最后 `complete`。
  - `uart_interrupt_handler`：轮询 LSR 的 DR 位（bit0）并从 RBR 取字节进行回显：
    - 换行：`'\r'`/`'\n'` → 输出 `\n`
    - 退格：`0x08` 或 `0x7f` → 输出 `"\x08 \x08"` 实现就地擦除。
    - 其他：按收到的 UTF-8 字节直出。
- 说明
  - UART 基本输出仍复用 `driver_uart` 的忙等发送（lab-1），本实验重点在"RX→中断→回显"的路径打通。

### 4. 时钟中断与全局系统时钟

- 全局时钟
  - 在 `kernel/src/trap/timer.rs` 维护 `SYS_TICKS: AtomicUsize`，提供 `create()/update()/get_ticks()` 三个操作。
- 基于 SBI 的 STIP 周期调度（默认路径）
  - 在 `inittraps_hart`：启用 `sstatus::sie` 与 `sie::{sext,ssoft,stimer}`，然后 `timer::start(hartid)`。
  - `timer::start`：仅在 hart0 调用 `program_next_tick()`，通过 SBI 的 `set_timer` 触发下一次 S 模式时钟中断。
  - `timer_interrupt_handler_stip`：hart0 上执行 `timer::update()`（ticks++），随后再次 `program_next_tick()` 注册下一次滴答。
  - `timer_interrupt_handler_ssip`：用于"从 M 模式路径转移来的软件中断"场景；本仓库默认以 SBI-STIP 为主，不依赖 SSIP。
- 预置的 M 模式定时器向量路径（备用/对照）
  - `vector.S` 中提供 `timer_vector_base/timer_vector`，`timer.rs::init` 会将 `mtvec` 设为向量表基址并打开 `mie.mtimer`。
  - 通过 `mscratch` 提供每核临时区，`timer_vector_body()` 读取 `mtimecmp` 地址与 `INTERVAL`，完成 `mtimecmp += INTERVAL` 并置位 `sip.ssoft()`，将控制权移交给 S 模式处理。

### 5. 多核初始化

- 在 `kernel/src/init/trap.rs::trap_init(hartid)` 中：
  - `inittraps()`：一次性全局初始化（PLIC 优先级、`timer::create()`、UART IER）。
  - `inittraps_hart(hartid)`：每核初始化（PLIC 使能与阈值、`stvec=kernel_vector`、`sscratch=hartid`、打开 S 模式中断开关、`timer::start(hartid)`）
