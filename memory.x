MEMORY
{
    FLASH : ORIGIN = 0x8000000, LENGTH = 252K
    STORAGE : ORIGIN = 0x803F000, LENGTH = 4K
    RAM : ORIGIN = 0x20000000, LENGTH = 62K
    WARM : ORIGIN = 0x2000F800, LENGTH = 2K
}
__storage = ORIGIN(STORAGE);
WARM_DATA = ORIGIN(WARM);
