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
A simple microkernel written in Rust for RISC-V architecture as a learning project.
## Lab-2 Report is [here](docs/lab-2.md)
## Roadmap
- [ ] Base Kernel
  - [x] Bootloader
  - [ ] Kernel Initialization
    - [x] CPU
    - [x] Memory
      - [x] Physical Memory Manager
      - [x] Virtual Memory Manager
    - [ ] Devices
      - [x] UART
      - [x] DTB
    - [ ] Interrupts
      - [ ] Timer
      - [ ] UART
  - [ ] Device Tree Support
    - [x] UART
    - [x] CPU
    - [x] Memory
  - [x] Memory Management
    - [x] Physical Memory
    - [x] Virtual Memory
  - [ ] Interrupt Handling
    - [ ] UART Interrupt
    - [ ] Timer Interrupt
- [ ] Devices Drivers
  - [x] UART
## Requirements
- Rust (latest stable version)
- QEMU (for RISC-V)
- RISC-V GNU Toolchain
- GNU Make
- GDB (for RISC-V)
## Getting Started
### Build the project
```sh
cargo xtask build
```
### Run in QEMU
```sh
cargo xtask run
```
### Run tests
```sh
cargo xtask test
```
### Debug with GDB
```sh
cargo xtask gdb
gdb-multiarch -ex "target remote :1234" -ex "set architecture riscv:rv64" -ex "file target/riscv64imac-unknown-none-elf/debug/glenda"
```
## Known Issues
- Nothing here

## Contributors
- [Mitchell Xu](https://github.com/zeyi2)
- [Vincent Wang](https://github.com/2018wzh)

## License
This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details

## Credits
- [Plan 9 from Bell Labs](https://plan9.io) for inspiration and the name and mascot "Glenda"
- [r9os](https://github.com/r9os/r9) for the build system xtask
