cmake_minimum_required(VERSION 3.10)
project(hello-c C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)

set(GLENDA_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../..")
set(LIBGLENDA_DIR "${GLENDA_ROOT}/lib/libglenda")


# Include directories
include_directories(
    "${GLENDA_ROOT}/target/include"
)

# Search for libraries in these locations
link_directories(
    "${GLENDA_ROOT}/target/lib"
)


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
    glenda
)

# Install rule (required by xtask build system)
install(TARGETS hello-c RUNTIME DESTINATION .)
