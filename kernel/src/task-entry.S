.global irq_entry
irq_entry:
    push 0
    push 32
    jmp common_entry
.global syscall_entry
syscall_entry:
    push 0
    push 128
    jmp common_entry
.global page_fault_entry
page_fault_entry:
    push 14
    jmp common_entry
.global protection_entry
protection_entry:
    push 13
    jmp common_entry
.global invalid_entry
invalid_entry:
    push 0
    push 6
    jmp common_entry
.global divide_entry
divide_entry:
    push 0
    push 0
common_entry:
    cld
    push rax
    push rbx
    push rcx
    push rdx
    push rbp
    push rdi
    push rsi
    push r8
    push r9
    push r10
    push r11
    push r12
    push r13
    push r14
    push r15
    mov rdi, rsp
    and rsp, -16
    call task_trap
    mov rsp, rax
    jmp restore_frame
.global enter_user
enter_user:
    mov rsp, rdi
restore_frame:
    pop r15
    pop r14
    pop r13
    pop r12
    pop r11
    pop r10
    pop r9
    pop r8
    pop rsi
    pop rdi
    pop rbp
    pop rdx
    pop rcx
    pop rbx
    pop rax
    add rsp, 16
    iretq
