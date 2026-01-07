/* Memory layout for RP2040 (SKR Pico) */
/* 2MB Flash, 264KB SRAM */

MEMORY {
    /* Boot ROM lives at the start of flash */
    BOOT2 : ORIGIN = 0x10000000, LENGTH = 0x100

    /* Application code starts after boot2 */
    FLASH : ORIGIN = 0x10000100, LENGTH = 2048K - 0x100 - 64K

    /* Last 64KB reserved for config storage */
    /* CONFIG : ORIGIN = 0x101F0000, LENGTH = 64K */

    /* SRAM is split into banks but can be used contiguously */
    RAM : ORIGIN = 0x20000000, LENGTH = 264K
}

/* Heap configuration */
/* Reserve some SRAM for heap (used by TOML parser) */
_heap_size = 32K;

/* Export symbols for the runtime */
EXTERN(BOOT2_FIRMWARE);
