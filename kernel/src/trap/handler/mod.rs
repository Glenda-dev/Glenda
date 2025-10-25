mod kernel;
mod user;

const EXCEPTION_INFO: [&str; 16] = [
    "Instruction address misaligned", // 0
    "Instruction access fault",       // 1
    "Illegal instruction",            // 2
    "Breakpoint",                     // 3
    "Load address misaligned",        // 4
    "Load access fault",              // 5
    "Store/AMO address misaligned",   // 6
    "Store/AMO access fault",         // 7
    "Environment call from U-mode",   // 8
    "Environment call from S-mode",   // 9
    "reserved-1",                     // 10
    "Environment call from M-mode",   // 11
    "Instruction page fault",         // 12
    "Load page fault",                // 13
    "reserved-2",                     // 14
    "Store/AMO page fault",           // 15
];

const INTERRUPT_INFO: [&str; 16] = [
    "U-mode software interrupt", // 0
    "S-mode software interrupt", // 1
    "reserved-1",                // 2
    "M-mode software interrupt", // 3
    "U-mode timer interrupt",    // 4
    "S-mode timer interrupt",    // 5
    "reserved-2",                // 6
    "M-mode timer interrupt",    // 7
    "U-mode external interrupt", // 8
    "S-mode external interrupt", // 9
    "reserved-3",                // 10
    "M-mode external interrupt", // 11
    "reserved-4",                // 12
    "reserved-5",                // 13
    "reserved-6",                // 14
    "reserved-7",                // 15
];
