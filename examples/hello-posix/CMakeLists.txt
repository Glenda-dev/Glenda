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

set(APE_LIB "${SYSROOT}/lib/libape.a")
set(LIBC_LIB "${SYSROOT}/lib/libc.a")

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
    "${LIBC_LIB}"
    "${APE_LIB}"
    -Wl,--end-group
    ${SYSROOT}/lib/crt1.o
    ${SYSROOT}/lib/crti.o
    ${SYSROOT}/lib/crtn.o
)

# Ensure linker reruns when static runtime archives are refreshed by xtask.
set_property(TARGET hello-posix APPEND PROPERTY LINK_DEPENDS "${LIBC_LIB};${APE_LIB}")

# Install rule (required by xtask build system)
install(TARGETS hello-posix RUNTIME DESTINATION .)
