
CC = $(CROSS_COMPILE)gcc
OBJCOPY = $(CROSS_COMPILE)objcopy

CFLAGS = -nostdlib -ffreestanding -fno-builtin -fno-stack-protector -march=rv64gc -mabi=lp64d -Os
LDFLAGS = -Wl,-n -Wl,--build-id=none -Wl,-Ttext=0
INCLUDES = -I../../include/kernel

SRCS = start.S hello.c
TARGET_DIR = $(CURDIR)/../../target/service/hello
TARGET_ELF = $(TARGET_DIR)/hello.elf
TARGET_BIN = $(TARGET_DIR)/hello.bin

.PHONY: all clean

all: $(TARGET_BIN)

$(TARGET_BIN): $(TARGET_ELF)
	$(OBJCOPY) -O binary $< $@

$(TARGET_ELF): $(SRCS)
	$(CC) $(CFLAGS) $(LDFLAGS) $(INCLUDES) -o $@ $^

clean:
	rm -f $(TARGET_ELF) $(TARGET_BIN)