
CC = $(CROSS_COMPILE)gcc
OBJCOPY = $(CROSS_COMPILE)objcopy
OBJDUMP = $(CROSS_COMPILE)objdump

CFLAGS = -nostdlib -ffreestanding -fno-builtin -fno-stack-protector -march=rv64gc -mabi=lp64d -Os
LDFLAGS = -Wl,-n -Wl,--build-id=none -Wl,-Tlink.ld
INCLUDES = -I../../include/kernel

SRCS = start.S hello.c string.c
TARGET_DIR = $(CURDIR)/../../target/service/hello
TARGET_ELF = $(TARGET_DIR)/hello.elf
TARGET_BIN = $(TARGET_DIR)/hello.bin
TARGET_DUMP = $(TARGET_DIR)/hello.dump

.PHONY: all clean

all: $(TARGET_BIN)

$(TARGET_BIN): $(TARGET_ELF)
	$(OBJCOPY) -O binary $< $@

$(TARGET_ELF): $(SRCS) link.ld
	$(CC) $(CFLAGS) $(LDFLAGS) $(INCLUDES) -o $@ $(SRCS)
$(TARGET_DUMP): $(TARGET_ELF)
	$(OBJDUMP) -D $< > $@

clean:
	rm -f $(TARGET_ELF) $(TARGET_BIN) $(TARGET_DUMP)
dump: $(TARGET_DUMP)
