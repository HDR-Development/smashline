.section .text.__smashline_interpreter, "ax", %progbits
.global __smashline_interpreter
.type __smashline_interpreter, %function
.hidden __smashline_interpreter
.align 2
.cfi_startproc
__smashline_interpreter:
    stp x29, x30, [sp, #-0x10]!
    stp x19, x20, [sp, #-0x10]!
    str x21, [sp, #-0x10]!
    mov x19, x0
    mov x20, x1
    mov x21, x2
    adr x0, __smashline_interpreter_landing_pad
    mov x1, sp
    bl set_smashline_interpreter_landing
    mov x0, x19
    mov x1, x20
    mov x2, x21
    bl smashline_interpreter
__smashline_interpreter_exit:
    bl clear_smashline_interpreter_landing
    ldr x21, [sp], #0x10
    ldp x19, x20, [sp], #0x10
    ldp x29, x30, [sp], #0x10
    ret

__smashline_interpreter_landing_pad:
    nop
    bl restore_smashline_interpreter_stack
    mov sp, x0
    b __smashline_interpreter_exit
.cfi_endproc