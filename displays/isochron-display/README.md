# isochron-display

Display abstraction layer for the Isochron firmware ecosystem.

This crate provides:
- Display backend traits for different display types
- Navigation event types for user input
- Screen buffer management
- Shared rendering utilities

## Display Types

### External Displays (Separate MCU)

External displays have their own MCU and communicate with the controller via UART:
- V0 Mini OLED (STM32F042 + SH1106)
- Future TFT displays

### Direct Displays

Direct displays connect to the controller's I2C/SPI bus:
- I2C OLED (SSD1306, SH1106)
- SPI OLED
- Future: SPI TFT

## Traits

### DisplayBackend

```rust
pub trait DisplayBackend {
    fn clear(&mut self) -> Result<(), DisplayError>;
    fn draw_text(&mut self, row: u8, col: u8, text: &str) -> Result<(), DisplayError>;
    fn invert_region(&mut self, row: u8, start_col: u8, end_col: u8) -> Result<(), DisplayError>;
    fn flush(&mut self) -> Result<(), DisplayError>;
    fn dimensions(&self) -> (u8, u8);
    fn is_ready(&self) -> bool;
}
```

### InputSource

```rust
pub trait InputSource {
    fn poll(&mut self) -> Option<NavigationEvent>;
    fn is_active(&self) -> bool;
}
```

## Navigation Events

```rust
pub enum NavigationEvent {
    ScrollUp,      // Encoder rotate CW / touch swipe up
    ScrollDown,    // Encoder rotate CCW / touch swipe down
    Select,        // Encoder press / touch tap
    Back,          // Back button / touch swipe left
    LongSelect,    // Long encoder press / touch long press
    Increment,     // Fine increment for value editing
    Decrement,     // Fine decrement for value editing
}
```

## Screen Buffer

The `Screen` type provides a text-mode buffer:

```rust
let mut screen = Screen::new(4, 21);  // 4 rows, 21 columns
screen.clear();
screen.set_line(0, "Temperature: 40.0C");
screen.set_line(1, "Status: Heating");
```

## Usage

Display implementations use this crate's traits:

```rust
use isochron_display::{DisplayBackend, NavigationEvent, InputSource};

struct MyOledDisplay { /* ... */ }

impl DisplayBackend for MyOledDisplay {
    fn clear(&mut self) -> Result<(), DisplayError> {
        // Clear display buffer
    }
    // ... other methods
}
```

## License

GPL-3.0-or-later
