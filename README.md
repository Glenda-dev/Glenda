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
## Usage
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
## Contributors
- [Mitchell Xu](https://github.com/zeyi2)
- [Vincent Wang](https://github.com/2018wzh)

## License
This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details

## Credits
- [Plan 9 from Bell Labs](https://plan9.io) for inspiration and the name and mascot "Glenda"
- [r9os](https://github.com/r9os/r9) for the build system xtask