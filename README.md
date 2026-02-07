# Glenda Kernel
```plaintext
                 __
                /  )
               /' /    __
        _.----'-./  _-"  )
      -"         "v"  _-'   $$$$$$\  $$\                           $$\
    ."             'Y"     $$  __$$\ $$ |                          $$ |
   |                |      $$ /  \__|$$ | $$$$$$\  $$$$$$$\   $$$$$$$ | $$$$$$\
   | o     o        |      $$ |$$$$\ $$ |$$  __$$\ $$  __$$\ $$  __$$ | \____$$\
   |  .><.          |      $$ |\_$$ |$$ |$$$$$$$$ |$$ |  $$ |$$ /  $$ | $$$$$$$ |
   |  "Ll"         /       $$ |  $$ |$$ |$$   ____|$$ |  $$ |$$ |  $$ |$$  __$$ |
   '.             |        \$$$$$$  |$$ |\$$$$$$$\ $$ |  $$ |\$$$$$$$ |\$$$$$$$ |
    |             |         \______/ \__| \_______|\__|  \__| \_______| \_______|
    \             )
    / .          /'\    *
    '-(_/,__.--^--"  *      *
                   *     *        *
```
Glenda 是一个基于 Rust 编写的跨架构的研究型微内核操作系统。它致力于结合 **seL4** 的形式化设计原则（基于 Capabilities 的强隔离机制）与 **Plan 9** 的分布式设计哲学（Everything is a file & Private Namespaces）。

## 项目亮点

- **微内核架构**: 内核仅提供最基本的资源管理机制，如线程管理、地址空间管理和物理内存分配。
- **能力安全机制 (Capability-based)**: 内核中所有资源均由 Capability 进行抽象，资源访问权限通过 CNode 传递，从设计上规避了 Confused Deputy 等安全风险。
- **基于 UTCB 的高性能 IPC**: 采用 User Thread Control Block 技术实现零拷贝消息传递，优化了同步 RPC 调用的系统开销。
- **现代化构建系统**: 基于 Rust 全栈开发的 `xtask` 自动化工具流，实现了内核镜像打包、文件系统构建及 QEMU 引导的一键化。

## 核心组件

- [kernel/](kernel/): 核心微内核，处理 Trap、调度、内存分配及 IPC。
- [lib/libglenda-rs/](lib/libglenda-rs/): 针对微内核环境优化的标准运行时环境。
- [service/](service/):
  - **Warren**: 系统根进程（Root Task），引导系统资源并协调初始化。
  - **Unicorn**: 用户态驱动管理框架。
  - **Nineball**: 系统引导进程。
- [drivers/](drivers/): 硬件驱动组件（如 VirtIO 磁盘与 UART）。

## 环境要求

- **Rust**: nightly 版本（需支持 `riscv64gc-unknown-none-elf` 目标）。
- **QEMU**: 推荐 8.0+ 版本，支持 `virt` 平台。
- **工具链**: `riscv64-unknown-elf-gdb` 或 `gdb-multiarch` 用于远程调试。

## 快速上手

本项目不支持使用 `cargo build`。请通过专用的构建任务进行操作：

```sh
# 编译并打包系统镜像
cargo xtask build

# 启动并引导 QEMU 环境
cargo xtask run

# 加载测试配置启动（例如 hello 示例）
cargo xtask --config config/hello.toml run

# 进入 GDB 远程调试模式
cargo xtask gdb
```

## 开发路线图 (Roadmap)

### 内核层
- [x] S-mode 基础引导与 SBI 交互
- [x] 物理内存管理器 (Untyped Memory)
- [x] 基于能力 (Capability) 的地址空间管理 (VSpace)
- [x] UTCB 增强型同步 IPC
- [x] 优先级抢占式调度器 (Scheduler)
- [x] 基础 Trap 处理与系统调用 (Syscall) 框架
- [ ] HAL 抽象与多架构支持
  - [x] RISC-V 64 支持
  - [ ] LoongArch 64 支持
- [ ] 硬件中断托管服务 (Async IRQ)

### 服务层
- [x] Warren 引导框架实现
- [ ] 9Ball 系统引导任务
- [ ] Unicorn 设备驱动管理器
- [ ] Fossil 命名空间服务器
- [ ] Gopher 网络栈
- [x] UART 字符设备驱动 (User-mode)
- [x] VirtIO Block 磁盘驱动
- [ ] VirtIO 网络驱动

## 贡献者

- [Mitchell Xu](https://github.com/zeyi2)
- [Vincent Wang](https://github.com/2018wzh)

## 参考项目与致谢

- [Plan 9 from Bell Labs](https://plan9.io): 精神内核与设计灵感。
- [seL4 Microkernel](https://sel4.systems): 对象能力模型设计参考。

## 许可证

本项目遵循 [MIT License](LICENSE)。
