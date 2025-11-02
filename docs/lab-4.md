# Lab-4 Report
## 实现目标
- 建立进程的基本框架
- 实现上下文切换处理函数
- 实现进程的创建与运行
- 实现系统调用机制
- 实现简单的用户态程序
- 编写测试用例验证功能正确性
## 具体步骤
### 1. 建立进程的基本框架
- 在`kernel/src/trap/context.rs`中定义陷阱帧结构体`TrapFrame`用来保存寄存器状态。
- 在`kernel/src/proc/context.rs`中定义进程上下文结构体`ProcContext`，用于保存进程的运行状态。
- 在`kernel/src/proc/process.rs`定义进程结构体`Process`，其中包括进程ID，页表地址，进程上下文，陷阱帧等信息。
### 2. 实现上下文切换处理函数
- 在`kernel/src/asm/switch.S`中实现`switch_context`汇编函数，用于实现上下文切换的具体逻辑。
- 在`kernel/src/asm/trampoline.S`中实现`user_vector`和`user_return`函数，用于处理从用户态到内核态的切换和返回。
    - `user_vector`负责保存用户态寄存器状态到陷阱帧，并切换到内核态。
    - `user_return`负责从陷阱帧恢复用户态寄存器状态，并切换回用户态。
- 在`kernel/src/linker.ld`中定义trampsec区域，用于在独立的页面存放trampoline代码。
### 3. 实现进程的创建与运行
- 在`kernel/src/proc/process.rs`中实现`create`函数，用于创建新进程，分配页表，初始化上下文和陷阱帧。
    * 首先初始化页表
    * 按照地址空间布局从低到高使用`vm_mappages`映射各个段
    * 将传入的用户程序代码复制到用户代码段
    * 设置栈指针，堆指针，入口点
- 在`kernel/src/proc/process.rs`中实现`launch`函数，用于将进程切换到用户态运行。
    * 设置进程的陷阱帧，保存当前内核状态
    * 设置TrapFrame的用户虚拟地址到sscratch寄存器用于trampoline调用
    * 设置内核态上下文
    * 将进程关联到对应的CPU
    * 调用`switch_context`切换到用户态
- 在`kernel/src/trap/handler/user/mod.rs`中实现`trap_user_handler`和`trap_user_return`函数，分别处理用户态的陷阱和返回。
    * `trap_user_handler`保存用户态寄存器状态到陷阱帧，并调用内核态的陷阱处理函数
        * 首先获取当前进程的陷阱帧
        * 保存用户态寄存器状态到陷阱帧
        * 调用内核态的陷阱处理函数
    * `trap_user_return`恢复用户态寄存器状态，并调用`user_return_fn`返回用户态
        * 获取当前进程的陷阱帧
        * 将stvec设置为用户态向量入口
        * 恢复寄存器状态
        * 保存寄存器状态到陷阱帧用于user_return_fn
        * 计算user_return_fn的地址并调用
### 4. 实现系统调用机制
- 在`kernel/src/trap/handler/user/syscall.rs`中实现系统调用处理函数`syscall_handler`。
    * 获取系统调用号和参数
    * 根据系统调用号调用对应的处理函数
    * 将返回值存储到a0寄存器
- 在`kernel/src/trap/kernel/mod.rs`中内核态陷阱处理函数中添加对系统调用的处理。
- 在`kernel/src/syscall/helloworld.rs`中实现一个简单的系统调用
    * 注册为SYS_helloworld
    * 打印"proczero: hello world!"
### 5. 实现简单的用户态程序
- 编写`include`目录，包含用户态程序的头文件。
    * 定义系统调用号
    * 声明系统调用函数原型
- 编写`services`目录，包含用户态程序的实现。
    * 实现`hello/start.S`，设置用户态程序的入口点
    * 实现`hello/hello`，调用SYS_helloworld系统调用打印信息。
    * 编写`Makefile`，用于编译用户态程序生成可执行文件`hello.bin`。
### 6. 编写测试用例验证功能正确性
- 在`tests`目录下编写测试用例，验证进程创建、运行和系统调用功能的正确性。
    * 将用户态程序嵌入源码中
    * 编写字节码测试用例用于fallback测试
- 修改编译系统，将测试用例编译为内核镜像的一部分。
    * 调用`make`编译用户态程序
    * 生成`proc_payload.rs`，将用户态程序嵌入内核源码
