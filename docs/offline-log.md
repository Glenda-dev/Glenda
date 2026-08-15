# Glenda 学生闭环仿真记录（离线阶段）

学生身份：李明 <liming@student.example.edu.cn>
项目：`D:\Workspace\glenda-spec`（参考源码 `.tmp/Glenda`，仅作为卡住时的修复依据，每次手动修复记录为 `spec/patches/*.yaml`）
LLM：ECNU Anthropic 兼容网关，模型 `ecnu-max`（`.vos/config.toml`，密钥经项目 `.env` 注入，不入库）

## 里程碑映射

| Stage | 范围 | 参考分支 | 状态 |
|---|---|---|---|
| M1 | 启动 + 串口：boot.S、printk、uart、spinlock，输出 GLENDA_BOOT_OK | lab-1 | 进行中 |
| M2 | DTB + 内存：dtb、pmem/vm、早期 trap | lab-2/3 | 待开始 |
| M3 | 中断 + 进程 + syscall：clint/plic、proc、scheduler、syscall、enter/switch/trampoline | lab-4~6 | 待开始 |
| M4 | FS + virtio：fs/*、virtio disk、disk.img 启动 | lab-7/8 | 待开始 |
| M5 | 用户态（candidate）：service/hello、首个用户进程、整机启动 | lab-9 | 待开始 |

## Spec 阶段记录

- `vos init` 生成五文件族模板（commit 38537b0）。
- `.gitattributes` 强制 LF（commit b4ceef4）。
- 手写 design.yaml + 11 个模块 + 4 个接口 + 1 个目标；`spec lint all` 通过（commit 8c5b352）。
- `vos agent review`（ecnu-max 真实调用）发现：console 所有权冲突、依赖环、userland owns 重复、toolchain 检查无绑定属性；逐条修正并再次 lint 通过（commit db37716）。
- 重要事实修正：Glenda 内核为纯 Rust（printk 走 drivers/uart crate 直接 MMIO，SBI 仅用于 timer/IPI），spec owns 已按真实布局对齐。

## 环境与工具链

- 宿主：cargo 1.99.0-nightly（target riscv64gc-unknown-none-elf 已装）、riscv64-unknown-elf-gcc 14.2.0（xPack，C:\tools）、QEMU 10.2.0、make 4.4.1。
- 网关探测：`ecnu-max` 裸模型名在 Bearer 与 x-api-key 两种头下均可用（脚本 /tmp/probe-ecnu.sh）。
- doctor：结构化诊断对 bun 工具绑定失败 50 轮，按 AGENTS.md 语义降级为 warning；网关连通性正常。

## 每里程碑记录格式

每个里程碑记录：agent 命令与结果摘要、手动修复清单（对应 spec/patches）、build/run/verify/report 输出摘要、证据文件路径。

## M0/M1 工具链里程碑记录（2026-08-15）

- `vos agent implement toolchain`（run 202608151116379-1c2e83e9，ecnu-max）：结构化提交经历 6 轮以上 schema 修复；权威校验的 boot-console-binding 需要 boot-asm 拥有的 kernel/src/printk.rs，超出 toolchain 所有权边界，循环不可收敛，按预案终止并手动落盘。
- 手动落盘（偏差记录：spec/patches/toolchain-manual-landing.yaml，commit 758c4b5）：Agent worktree 产物已离线验证——`vos build` 通过且 submittable；`vos run qemu` oracle 精确命中一次 GLENDA_BOOT_OK；离线 clean rebuild 字节级一致（kernel sha256 8cde4cf5fde6e045950b974778abe7667c1d703a8ae33946585f45435a8c4d3b）。
- `vos verify` 记录当前预期状态：toolchain 四项检查 CHECK_OK；boot-console-binding 待 M1 boot-asm 实现后自然通过；submittable:false（预期内，M1 范围未满足）。
- 证据：.vos/runs/202608151116379-1c2e83e9/（events.jsonl、manifest.json）。

## M5 里程碑记录（用户态 candidate，参考 refs/tmp/lab-9）

- 落盘方式：手动（spec/patches/glenda-m5-manual-landing.yaml）。orphan 工作分支 work/glenda-m5 自 refs/tmp/lab-9 检出，叠加 master 学生脚手架。
- 上游构建缺口（诚实修复 1）：service/hello/Makefile 的 SRCS 引用了树中不存在的 string.c，make 直接失败；hello.c 在 -ffreestanding 下需要编译器隐式生成的 memset/memcpy（test_buffer/lab9_test_2 的链接错误可证）。学生补写最小 service/hello/string.c（memset/memcpy/memmove）后 payload 链接通过。
- 课程适配（诚实修复 2）：lab-9 上游从不打印 LOGO；kernel/src/main.rs init() 后新增 AtomicBool 一次性横幅块（LOGO + GLENDA_BOOT_OK，显式 \n）。
- 构建：`cargo xtask build` 通过（上游存量 warning 较多）；hello.elf/hello.bin 由 make 生成并经 xtask 写入 target/proc_payload.rs。
- 启动：QEMU virt + disk.img(virtio-blk)，GLENDA_BOOT_OK 恰好 1 次；payload 被调度并跑完 LAB-9 自检（[ALL PASS] LAB-9 tests completed.）。
- 里程碑检查：boot-banner-public / userland-exec-public / userland-evidence-public；本地复跑后两项 CHECK_OK。
- 候选语义：M5 仅落 candidate（物理板证据与人工评审缺失，且上游自带构建缺口），标签 course/glenda-m5-candidate，不使用 -complete。
