OUTPUT_FORMAT(elf64-x86-64)
ENTRY(user_start)
PHDRS { text PT_LOAD; rodata PT_LOAD; data PT_LOAD; }
SECTIONS {
    . = 0x400000;
    .text : { *(.text .text.*) } :text
    . = ALIGN(4096);
    .rodata : { *(.rodata .rodata.*) } :rodata
    . = ALIGN(4096);
    .data : { *(.data .data.*) *(.got .got.*) } :data
    .bss : { *(.bss .bss.*) *(COMMON) } :data
    /DISCARD/ : { *(.eh_frame*) *(.note .note.*) }
}
