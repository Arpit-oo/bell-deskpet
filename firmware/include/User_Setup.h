#ifndef USER_SETUP_LOADED
#define USER_SETUP_LOADED
#endif

#define USER_SETUP_ID 42
// Driver can be overridden per-env via build flags (e.g. -D ILI9341_2_DRIVER)
#if !defined(ILI9341_2_DRIVER) && !defined(ST7789_DRIVER)
#define ILI9341_DRIVER
#endif
#define TFT_MISO 19
#define TFT_MOSI 23
#define TFT_SCLK 18
#define TFT_CS    5
#define TFT_DC   27
#define TFT_RST  33
#define TFT_ROTATION 0
#define LOAD_GLCD
#define LOAD_FONT2
#define LOAD_FONT4
#define LOAD_FONT6
#define LOAD_FONT7
#define LOAD_FONT8
#define LOAD_GFXFF
#define SMOOTH_FONT
#ifndef SPI_FREQUENCY
#define SPI_FREQUENCY  40000000
#endif
#define SPI_READ_FREQUENCY  16000000
#ifndef TFT_RGB_ORDER
#define TFT_RGB_ORDER TFT_BGR
#endif
#if !defined(TFT_INVERSION_ON)
#define TFT_INVERSION_OFF
#endif
