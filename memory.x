MEMORY
{
    FLASH : ORIGIN = 0x8000000, LENGTH = 252K
    STORAGE : ORIGIN = 0x803F000, LENGTH = 4K
    RAM : ORIGIN = 0x20000000, LENGTH = 64K
}
__storage = ORIGIN(STORAGE);