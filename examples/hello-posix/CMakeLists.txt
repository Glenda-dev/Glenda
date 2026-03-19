cmake_minimum_required(VERSION 3.10)
project(hello-posix C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)

set(GLENDA_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../..")
set(SYSROOT "${GLENDA_ROOT}/target/sysroot")

# Use LLVM/Clang toolchain and hard-float API (lp64d)
set(CMAKE_C_COMPILER clang)
set(CMAKE_ASM_COMPILER clang)

# Include directories from musl-glenda sysroot
include_directories(
    "${SYSROOT}/include"
)

# Search for libraries in musl-glenda sysroot
link_directories(
    "${SYSROOT}/lib"
)

# Create executable
add_executable(hello-posix main.c)
set_target_properties(hello-posix PROPERTIES OUTPUT_NAME "hello-posix.elf")

# Compiler flags for POSIX/musl
target_compile_options(hello-posix PRIVATE
    -Wall -Wextra -Wno-unused-parameter
    -ffreestanding -nostdinc
)


# Linker flags for POSIX/musl
target_link_options(hello-posix PRIVATE
    -static -nostdlib
    -fuse-ld=lld
)

# Link against musl, ape (for emulation syscall translation), and gcc support
# Order matters: crt1.o and other objects are usually added by the build system
# but in our custom nostdlib environment, we link them explicitly if needed.
# For hello-posix, we link the static libc.a from musl.
target_link_libraries(hello-posix PRIVATE
    -Wl,--start-group
    c
    ape
    -Wl,--end-group
    ${SYSROOT}/lib/crt1.o
    ${SYSROOT}/lib/crti.o
    ${SYSROOT}/lib/crtn.o
)

# Install rule (required by xtask build system)
install(TARGETS hello-posix RUNTIME DESTINATION .)
