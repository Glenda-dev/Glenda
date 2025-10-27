.section .text
.globl hart_get_tp
hart_get_tp:
    mv a0, tp
    ret

