section .multiboot_header
header_start:
  ; Magic
  dd 0xe85250d6
  ; Arch
  dd 0
  ; Header length
  dd header_end - header_start
  ; Checksum
  dd 0x100000000 - (0xe85250d6 + 0 + (header_end - header_start))

  ; Optional multiboot tags
  ; Currently none

  ; End tag
  dw 0 ; Type
  dw 0 ; Flags
  dd 8 ; Size
header_end:
