cmake_minimum_required(VERSION 3.10)
project(hello-c C)

set(GLENDA_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../..")
set(LIBGLENDA_DIR "${GLENDA_ROOT}/lib/libglenda")
set(MUSL_DIR "${GLENDA_ROOT}/lib/musl-glenda")

# Architecture selection
if(NOT ARCH)
    set(ARCH "riscv64")
endif()

# Include directories
include_directories(
    "${LIBGLENDA_DIR}/include"
    "${MUSL_DIR}/include"
    "${MUSL_DIR}/obj/include"
    "${MUSL_DIR}/arch/${ARCH}"
    "${MUSL_DIR}/arch/generic"
)

# Search for libraries in these locations
link_directories(
    "${LIBGLENDA_DIR}/build"
    "${MUSL_DIR}/lib"
)

# CRT objects from musl
set(CRT1_OBJ "${MUSL_DIR}/lib/crt1.o")
set(CRTI_OBJ "${MUSL_DIR}/lib/crti.o")
set(CRTN_OBJ "${MUSL_DIR}/lib/crtn.o")

# Create executable
add_executable(hello-c main.c)

# Compiler flags
target_compile_options(hello-c PRIVATE
    -Wall -Wextra -Wno-unused-parameter
    -fno-builtin -ffreestanding -nostdinc
)

# Linker flags
target_link_options(hello-c PRIVATE
    -static -nostdlib
    -T${GLENDA_ROOT}/lib/libglenda-rs/src/arch/${ARCH}/linker.ld
)

# Link libraries and CRT objects
# We use absolute paths for CRT objects to ensure placement
target_link_libraries(hello-c PRIVATE
    "${CRT1_OBJ}"
    "${CRTI_OBJ}"
    -Wl,--start-group
    glenda
    c
    gcc
    -Wl,--end-group
    "${CRTN_OBJ}"
)

# Install rule (required by xtask build system)
install(TARGETS hello-c RUNTIME DESTINATION .)
