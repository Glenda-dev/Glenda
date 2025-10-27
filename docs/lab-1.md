# Lab-1 Report

## 实现目标

- 初始化总体代码仓库结构
- 编写linker.ld和boot.S来设置内核栈和跳转到Rust内核入口函数glenda_main
- 实现DTB解析功能
- 初始化多核支持
- 实现UART驱动
- 在lock中实现自旋锁spinlock
- 实现输出函数printk，使用spin-rs库中的自旋锁防止输出错乱
- 在glenda_main中打印banner
- 编写输出与自旋锁测试

## 具体步骤

### 1. 初始化总体代码仓库结构

- 初始化git仓库与文件夹结构
- 编写Cargo.toml，配置交叉编译工具链和依赖
- 编写编译脚本xtask，支持编译、运行、测试和调试内核

### 2. 使用boot.S来设置内核栈和跳转到Rust内核入口函数glenda_main

- 在linker.ld中定义内核代码布局，将基地址设置为0x80200000以兼容OpenSBI，并将入口函数设置为_start
- 在boot.S中设置内核栈，暂时将中断处理函数设置为wfi等待，并以尾调用的方式跳转到glenda_main，并传入hartid和dtb地址参数
- 编写build.rs脚本，编译boot.S并将生成的目标文件链接到最终的内核映像中

### 3. 实现DTB解析功能

- 添加fdt依赖以支持DTB解析
- 编写dtb.rs模块，使用fdt库解析传入的DTB地址，并封装数据结构以便后续使用
- 在glenda_main中调用dtb模块进行DTB解析

### 4. 初始化多核支持

- 编写sbi_hart_start函数，调用OpenSBI的接口启动其他核
- 编写bootstrap_secondary_harts来使用sbi_hart_start启动其他核
- 在init中调用bootstrap_secondary_harts以启动多核

### 5. 实现UART驱动

- 编写uart.rs模块，实现UART初始化和字符输出功能
- 在dtb.rs中实现UART地址的解析
- 在uart.rs中使用解析后的UART地址进行初始化

### 6. 在lock中实现自旋锁spinlock

- 编写spinlock.rs，在SpinLock结构体中使用AtomicBool实现自旋锁的基本功能
- 使用atomic操作确保自旋锁的正确性和性能
- 在空闲中使用spin_loop释放CPU资源

### 7. 实现输出函数printk，使用spin-rs库中的自旋锁防止输出错乱
- 添加spin依赖以使用spin-rs库中的自旋锁
- 编写printk.rs模块，使用rust的format宏实现格式化输出
- 在printk.rs中使用spin::Mutex保护输出，防止多核同时输出错乱，在开始输出时加锁，在退出时自动释放锁资源（为保证健壮性，未使用自己实现的自旋锁）

### 8. 在glenda_main中打印banner
- 将banner信息存储在常量中，存储于logo.rs文件中
- 在glenda_main中在hart0初始化后调用printk打印内核banner信息

### 9. 编写输出与自旋锁测试
- 在tests目录下编写printk.rs测试用例，测试printk的基本输出功能，包括颜色转义符输出
- 在tests目录下编写spinlock.rs测试用例，在多核上同时运行测试，验证自旋锁的一致性