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

### 核心开发 (Done)
- [x] **内核基础**: S-mode 引导、SBI 交互、Trap 框架。
- [x] **能力系统 (Cap)**: Untyped 内存管理、CNode 权限传递、VSpace 地址空间控制。
- [x] **高性能 IPC**: 基于 UTCB 的同步消息传递。
- [x] **进程管理**: 优先级抢占式调度、Warren 根进程引导。
- [x] **驱动基础**: UART 用户态驱动、VirtIO Block 磁盘驱动。
- [x] **工具链**: `xtask` 自动化构建、镜像生成与 QEMU 自动化。

### 阶段 1: 系统服务增强 (In Progress)
- [ ] **Nineball**: 系统服务编排与自动化发现。
- [ ] **Fossil**: 全局命名空间文件系统。
- [ ] **Gopher**: 基于 LWIP 的用户态网络栈。
- [ ] **Unicorn**: 统一设备驱动管理模型。
- [ ] **Prism**: 基础图形显示驱动。
- [ ] **APE**: 初始 POSIX 兼容层支持。

### 阶段 2: 架构扩展与分布式 (Planned)
- [ ] **多架构支持**: 实现 LoongArch 64 端口。
- [ ] **Portal**: 远程 IPC 协议、USB4 隧道、RoCE 传输支持。
- [ ] **March**: 增强型工作池调度器与实时性约束。
- [ ] **Factotum**: 端到端身份验证与安全令牌管理。
- [ ] **Nexus**: 跨节点物理资源虚拟化与聚合。

### 阶段 3: 优化与高级特性
- [ ] **性能**: vDSO SeqLock 免 IPC 状态读取。
- [ ] **内存**: 写时复制 (COW) Fork 支持。
- [ ] **鲁棒性**: 异步 IRQ 托管与故障恢复机制。
- [ ] **分布式**: 进程/线程跨节点热迁移支持。

## 贡献者

- [Mitchell Xu](https://github.com/zeyi2)
- [Vincent Wang](https://github.com/2018wzh)

## 参考项目与致谢

- [Plan 9 from Bell Labs](https://plan9.io): 精神内核与设计灵感。
- [seL4 Microkernel](https://sel4.systems): 对象能力模型设计参考。

## 许可证

本项目遵循 [MIT License](LICENSE)。
