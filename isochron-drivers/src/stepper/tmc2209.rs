//! TMC2209 stepper driver (UART mode)
//!
//! The TMC2209 is a stepper motor driver with UART configuration interface
//! and features like StealthChop (quiet operation) and StallGuard (stall detection).
//!
//! # UART Protocol
//!
//! The TMC2209 uses a single-wire UART protocol at 115200 baud (8N1):
//! - Sync byte: 0x05
//! - Slave address: 0x00 (MS1/MS2 pins set the address 0-3)
//! - Register address + R/W bit
//! - Data (4 bytes, big-endian)
//! - CRC8
//!
//! # Features Used
//!
//! - StealthChop: Quiet operation mode using voltage chopping
//! - StallGuard: Load-based stall detection without physical endstops
//! - CoolStep: Dynamic current scaling based on load (optional)

use isochron_core::traits::{Direction, StepperDriver};

/// TMC2209 Register addresses
pub mod reg {
    /// General configuration
    pub const GCONF: u8 = 0x00;
    /// Global status flags
    pub const GSTAT: u8 = 0x01;
    /// Interface transmission counter
    pub const IFCNT: u8 = 0x02;
    /// Hold/run current settings
    pub const IHOLD_IRUN: u8 = 0x10;
    /// Power down delay
    pub const TPOWERDOWN: u8 = 0x11;
    /// Measured time between steps
    pub const TSTEP: u8 = 0x12;
    /// Upper velocity for StealthChop
    pub const TPWMTHRS: u8 = 0x13;
    /// Lower velocity for CoolStep/StallGuard
    pub const TCOOLTHRS: u8 = 0x14;
    /// StallGuard threshold
    pub const SGTHRS: u8 = 0x40;
    /// StallGuard result
    pub const SG_RESULT: u8 = 0x41;
    /// CoolStep configuration
    pub const COOLCONF: u8 = 0x42;
    /// Microstep counter
    pub const MSCNT: u8 = 0x6A;
    /// Chopper configuration
    pub const CHOPCONF: u8 = 0x6C;
    /// Driver status
    pub const DRV_STATUS: u8 = 0x6F;
    /// StealthChop PWM configuration
    pub const PWMCONF: u8 = 0x70;
}

/// UART sync byte for TMC2209
const SYNC_BYTE: u8 = 0x05;

/// TMC2209 driver configuration
#[derive(Debug, Clone)]
pub struct Tmc2209Config {
    /// UART address (0-3, set by MS1/MS2 pins)
    pub uart_address: u8,
    /// Run current in mA (100-2000)
    pub run_current_ma: u16,
    /// Hold current in mA (typically 50% of run current)
    pub hold_current_ma: u16,
    /// Enable StealthChop mode (quiet operation)
    pub stealthchop: bool,
    /// StallGuard threshold (0-255, lower = more sensitive)
    pub stallguard_threshold: u8,
    /// Microstepping (1, 2, 4, 8, 16, 32, 64, 128, 256)
    pub microsteps: u16,
}

impl Default for Tmc2209Config {
    fn default() -> Self {
        Self {
            uart_address: 0,
            run_current_ma: 800,
            hold_current_ma: 400,
            stealthchop: true,
            stallguard_threshold: 80,
            microsteps: 16,
        }
    }
}

impl Tmc2209Config {
    /// Convert microsteps to MRES register value
    pub fn mres(&self) -> u8 {
        match self.microsteps {
            256 => 0,
            128 => 1,
            64 => 2,
            32 => 3,
            16 => 4,
            8 => 5,
            4 => 6,
            2 => 7,
            1 => 8,
            _ => 4, // Default to 16 microsteps
        }
    }

    /// Convert current in mA to IRUN/IHOLD register value (0-31)
    /// Based on Rsense = 0.11 ohm (typical for TMC2209 breakout boards)
    pub fn current_to_cs(current_ma: u16) -> u8 {
        // CS = (I_rms * 32 * 1.41 * Rsense) / Vref - 1
        // With Rsense = 0.11, Vref = 0.325 (internal)
        // CS ≈ (I_rms * 32 * 1.41 * 0.11) / 0.325 - 1
        // CS ≈ I_rms * 15.34 - 1
        // For milliamps: CS = (I_mA * 1534 / 100000) - 1
        // For 800mA: CS ≈ (800 * 1534 / 100000) - 1 = 12 - 1 = 11
        let cs = ((current_ma as u32) * 1534 / 100000).saturating_sub(1);
        (cs.min(31)) as u8
    }
}

/// CRC8 calculation for TMC2209 UART
///
/// Uses polynomial 0x07 (x^8 + x^2 + x + 1) as specified by TMC2209.
pub fn crc8(data: &[u8]) -> u8 {
    let mut crc = 0u8;
    for &byte in data {
        let mut current = byte;
        for _ in 0..8 {
            if ((crc >> 7) ^ (current >> 7)) != 0 {
                crc = (crc << 1) ^ 0x07;
            } else {
                crc <<= 1;
            }
            current <<= 1;
        }
    }
    crc
}

/// Build a write datagram for TMC2209
pub fn build_write_datagram(addr: u8, reg: u8, data: u32) -> [u8; 8] {
    let mut datagram = [0u8; 8];
    datagram[0] = SYNC_BYTE;
    datagram[1] = addr;
    datagram[2] = reg | 0x80; // Set write bit
    datagram[3] = (data >> 24) as u8;
    datagram[4] = (data >> 16) as u8;
    datagram[5] = (data >> 8) as u8;
    datagram[6] = data as u8;
    datagram[7] = crc8(&datagram[..7]);
    datagram
}

/// Build a read request datagram for TMC2209
pub fn build_read_request(addr: u8, reg: u8) -> [u8; 4] {
    let mut datagram = [0u8; 4];
    datagram[0] = SYNC_BYTE;
    datagram[1] = addr;
    datagram[2] = reg; // No write bit
    datagram[3] = crc8(&datagram[..3]);
    datagram
}

/// Parse a read response from TMC2209
///
/// The TMC2209 responds with:
/// - Sync (0x05)
/// - Master address (0xFF)
/// - Register address
/// - 4 bytes data (big-endian)
/// - CRC8
///
/// Returns Ok(data) if valid, Err if CRC mismatch or invalid sync.
pub fn parse_read_response(response: &[u8; 8]) -> Result<u32, Tmc2209Error> {
    // Verify sync byte
    if response[0] != SYNC_BYTE {
        return Err(Tmc2209Error::InvalidSync);
    }

    // Verify CRC
    let expected_crc = crc8(&response[..7]);
    if response[7] != expected_crc {
        return Err(Tmc2209Error::CrcMismatch);
    }

    // Extract 32-bit data (big-endian)
    let data = ((response[3] as u32) << 24)
        | ((response[4] as u32) << 16)
        | ((response[5] as u32) << 8)
        | (response[6] as u32);

    Ok(data)
}

/// TMC2209 communication errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Tmc2209Error {
    /// Invalid sync byte in response
    InvalidSync,
    /// CRC mismatch
    CrcMismatch,
    /// Communication timeout
    Timeout,
}

/// Parsed DRV_STATUS register
#[derive(Debug, Clone, Copy, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct DrvStatus {
    /// StallGuard result (0-510)
    pub sg_result: u16,
    /// Motor standstill indicator
    pub standstill: bool,
    /// Overtemperature pre-warning (120°C)
    pub ot_prewarning: bool,
    /// Overtemperature shutdown (150°C)
    pub ot_shutdown: bool,
    /// Short to ground on phase A
    pub s2ga: bool,
    /// Short to ground on phase B
    pub s2gb: bool,
    /// Short to supply on phase A
    pub s2vsa: bool,
    /// Short to supply on phase B
    pub s2vsb: bool,
    /// Open load on phase A
    pub ola: bool,
    /// Open load on phase B
    pub olb: bool,
    /// StealthChop active
    pub stealth: bool,
    /// Current scaling (0-31)
    pub cs_actual: u8,
}

impl DrvStatus {
    /// Parse from raw DRV_STATUS register value
    pub fn from_register(value: u32) -> Self {
        Self {
            sg_result: (value & 0x3FF) as u16,
            standstill: (value & (1 << 31)) != 0,
            ot_prewarning: (value & (1 << 26)) != 0,
            ot_shutdown: (value & (1 << 25)) != 0,
            s2ga: (value & (1 << 24)) != 0,
            s2gb: (value & (1 << 23)) != 0,
            s2vsa: (value & (1 << 12)) != 0,
            s2vsb: (value & (1 << 11)) != 0,
            ola: (value & (1 << 29)) != 0,
            olb: (value & (1 << 30)) != 0,
            stealth: (value & (1 << 14)) != 0,
            cs_actual: ((value >> 16) & 0x1F) as u8,
        }
    }

    /// Check if any fault condition is present
    pub fn has_fault(&self) -> bool {
        self.ot_shutdown || self.s2ga || self.s2gb || self.s2vsa || self.s2vsb
    }

    /// Check if driver is in warning state
    pub fn has_warning(&self) -> bool {
        self.ot_prewarning || self.ola || self.olb
    }
}

/// TMC2209 driver state
///
/// This struct manages the driver state and provides methods for
/// configuring the TMC2209 over UART.
pub struct Tmc2209Driver {
    config: Tmc2209Config,
    current_rpm: u16,
    target_rpm: u16,
    direction: Direction,
    enabled: bool,
    stalled: bool,
    initialized: bool,
}

impl Tmc2209Driver {
    /// Create a new TMC2209 driver
    pub fn new(config: Tmc2209Config) -> Self {
        Self {
            config,
            current_rpm: 0,
            target_rpm: 0,
            direction: Direction::Clockwise,
            enabled: false,
            stalled: false,
            initialized: false,
        }
    }

    /// Get the configuration
    pub fn config(&self) -> &Tmc2209Config {
        &self.config
    }

    /// Build GCONF register value
    fn build_gconf(&self) -> u32 {
        let mut gconf = 0u32;

        // Bit 0: I_scale_analog = 0 (use internal reference)
        // Bit 1: internal_Rsense = 0 (external sense resistors)
        // Bit 2: en_spreadcycle = !stealthchop
        if !self.config.stealthchop {
            gconf |= 1 << 2;
        }
        // Bit 3: shaft = 0 (normal direction)
        // Bit 4: index_otpw = 0
        // Bit 5: index_step = 0
        // Bit 6: pdn_disable = 1 (disable PDN_UART input)
        gconf |= 1 << 6;
        // Bit 7: mstep_reg_select = 1 (use MSTEP register for microsteps)
        gconf |= 1 << 7;
        // Bit 8: multistep_filt = 1 (filter step pulses)
        gconf |= 1 << 8;

        gconf
    }

    /// Build CHOPCONF register value
    fn build_chopconf(&self) -> u32 {
        let mut chopconf = 0u32;

        // TOFF = 5 (off time, must be > 0 for driver to work)
        chopconf |= 5;
        // HSTRT = 4 (hysteresis start)
        chopconf |= 4 << 4;
        // HEND = 0 (hysteresis end)
        // TBL = 2 (blanking time)
        chopconf |= 2 << 15;
        // MRES = microstep resolution
        chopconf |= (self.config.mres() as u32) << 24;
        // intpol = 1 (interpolate to 256 microsteps)
        chopconf |= 1 << 28;
        // dedge = 0 (step on rising edge only)
        // diss2g = 0 (short to GND protection on)
        // diss2vs = 0 (short to VS protection on)

        chopconf
    }

    /// Build IHOLD_IRUN register value
    fn build_ihold_irun(&self) -> u32 {
        let ihold = Tmc2209Config::current_to_cs(self.config.hold_current_ma);
        let irun = Tmc2209Config::current_to_cs(self.config.run_current_ma);
        let iholddelay = 6u32; // Delay before reducing to hold current

        ((iholddelay & 0x0F) << 16) | ((irun as u32 & 0x1F) << 8) | (ihold as u32 & 0x1F)
    }

    /// Build PWMCONF register value for StealthChop
    fn build_pwmconf(&self) -> u32 {
        let mut pwmconf = 0u32;

        // PWM_OFS = 36 (offset amplitude)
        pwmconf |= 36;
        // PWM_GRAD = 14 (gradient amplitude)
        pwmconf |= 14 << 8;
        // PWM_FREQ = 1 (23.4kHz)
        pwmconf |= 1 << 16;
        // PWM_AUTOSCALE = 1
        pwmconf |= 1 << 18;
        // PWM_AUTOGRAD = 1
        pwmconf |= 1 << 19;
        // freewheel = 0 (normal operation)
        // PWM_REG = 4
        pwmconf |= 4 << 24;
        // PWM_LIM = 12
        pwmconf |= 12 << 28;

        pwmconf
    }

    /// Get register write datagrams for initialization
    ///
    /// Returns an array of datagrams to send over UART.
    pub fn init_datagrams(&self) -> [[u8; 8]; 6] {
        let addr = self.config.uart_address;

        [
            // GCONF - general configuration
            build_write_datagram(addr, reg::GCONF, self.build_gconf()),
            // CHOPCONF - chopper configuration + microsteps
            build_write_datagram(addr, reg::CHOPCONF, self.build_chopconf()),
            // IHOLD_IRUN - current settings
            build_write_datagram(addr, reg::IHOLD_IRUN, self.build_ihold_irun()),
            // TPOWERDOWN - power down delay
            build_write_datagram(addr, reg::TPOWERDOWN, 20),
            // PWMCONF - StealthChop configuration
            build_write_datagram(addr, reg::PWMCONF, self.build_pwmconf()),
            // SGTHRS - StallGuard threshold
            build_write_datagram(addr, reg::SGTHRS, self.config.stallguard_threshold as u32),
        ]
    }

    /// Mark as initialized
    pub fn set_initialized(&mut self) {
        self.initialized = true;
    }

    /// Check if initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Update stall status
    pub fn set_stalled(&mut self, stalled: bool) {
        self.stalled = stalled;
    }

    /// Sync current RPM with target (called when acceleration complete)
    pub fn sync_rpm(&mut self) {
        self.current_rpm = self.target_rpm;
    }

    /// Get read request for DRV_STATUS register
    pub fn read_status_request(&self) -> [u8; 4] {
        build_read_request(self.config.uart_address, reg::DRV_STATUS)
    }

    /// Get read request for TSTEP register (measures actual step rate)
    pub fn read_tstep_request(&self) -> [u8; 4] {
        build_read_request(self.config.uart_address, reg::TSTEP)
    }

    /// Get read request for IFCNT register (interface counter)
    pub fn read_ifcnt_request(&self) -> [u8; 4] {
        build_read_request(self.config.uart_address, reg::IFCNT)
    }

    /// Build a datagram to update run current
    pub fn set_current_datagram(&self, run_ma: u16, hold_ma: u16) -> [u8; 8] {
        let ihold = Tmc2209Config::current_to_cs(hold_ma);
        let irun = Tmc2209Config::current_to_cs(run_ma);
        let iholddelay = 6u32;
        let value =
            ((iholddelay & 0x0F) << 16) | ((irun as u32 & 0x1F) << 8) | (ihold as u32 & 0x1F);
        build_write_datagram(self.config.uart_address, reg::IHOLD_IRUN, value)
    }

    /// Build a datagram to update StallGuard threshold
    pub fn set_stallguard_datagram(&self, threshold: u8) -> [u8; 8] {
        build_write_datagram(self.config.uart_address, reg::SGTHRS, threshold as u32)
    }
}

impl StepperDriver for Tmc2209Driver {
    fn set_rpm(&mut self, rpm: u16) {
        self.target_rpm = rpm;
    }

    fn get_rpm(&self) -> u16 {
        self.target_rpm
    }

    fn set_direction(&mut self, dir: Direction) {
        self.direction = dir;
    }

    fn get_direction(&self) -> Direction {
        self.direction
    }

    fn enable(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn is_stalled(&self) -> bool {
        self.stalled
    }

    fn clear_stall(&mut self) {
        self.stalled = false;
    }

    fn is_at_speed(&self) -> bool {
        self.current_rpm == self.target_rpm
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mres_conversion() {
        let mut config = Tmc2209Config::default();

        config.microsteps = 256;
        assert_eq!(config.mres(), 0);

        config.microsteps = 16;
        assert_eq!(config.mres(), 4);

        config.microsteps = 1;
        assert_eq!(config.mres(), 8);
    }

    #[test]
    fn test_current_conversion() {
        // 800mA should give roughly CS=11
        let cs = Tmc2209Config::current_to_cs(800);
        assert!(cs >= 10 && cs <= 13);

        // 400mA should be lower
        let cs_low = Tmc2209Config::current_to_cs(400);
        assert!(cs_low < cs);
    }

    #[test]
    fn test_crc8() {
        // Test with known values
        let data = [0x05, 0x00, 0x00];
        let crc = crc8(&data);
        // CRC should be non-zero for this data
        assert!(crc != 0);
    }

    #[test]
    fn test_write_datagram() {
        let datagram = build_write_datagram(0, reg::GCONF, 0x00000140);

        assert_eq!(datagram[0], SYNC_BYTE);
        assert_eq!(datagram[1], 0); // address
        assert_eq!(datagram[2], reg::GCONF | 0x80); // register + write bit
                                                    // Data bytes in big-endian
        assert_eq!(datagram[3], 0x00);
        assert_eq!(datagram[4], 0x00);
        assert_eq!(datagram[5], 0x01);
        assert_eq!(datagram[6], 0x40);
        // CRC is calculated
        assert_eq!(datagram[7], crc8(&datagram[..7]));
    }

    #[test]
    fn test_init_datagrams() {
        let config = Tmc2209Config::default();
        let driver = Tmc2209Driver::new(config);

        let datagrams = driver.init_datagrams();
        assert_eq!(datagrams.len(), 6);

        // All should have sync byte
        for dg in &datagrams {
            assert_eq!(dg[0], SYNC_BYTE);
        }
    }

    #[test]
    fn test_driver_state() {
        let config = Tmc2209Config::default();
        let mut driver = Tmc2209Driver::new(config);

        assert!(!driver.is_enabled());
        assert!(!driver.is_stalled());
        assert!(!driver.is_initialized());

        driver.enable(true);
        assert!(driver.is_enabled());

        driver.set_rpm(120);
        assert_eq!(driver.get_rpm(), 120);

        driver.set_direction(Direction::CounterClockwise);
        assert_eq!(driver.get_direction(), Direction::CounterClockwise);
    }

    #[test]
    fn test_drv_status_parsing() {
        // Test standstill flag (bit 31)
        let status = DrvStatus::from_register(0x80000000);
        assert!(status.standstill);
        assert!(!status.has_fault());

        // Test StallGuard result (bits 0-9)
        let status = DrvStatus::from_register(0x000001FF);
        assert_eq!(status.sg_result, 0x1FF);

        // Test overtemperature shutdown (bit 25)
        let status = DrvStatus::from_register(1 << 25);
        assert!(status.ot_shutdown);
        assert!(status.has_fault());

        // Test StealthChop active (bit 14)
        let status = DrvStatus::from_register(1 << 14);
        assert!(status.stealth);

        // Test CS_ACTUAL (bits 16-20)
        let status = DrvStatus::from_register(0x001F0000);
        assert_eq!(status.cs_actual, 31);
    }

    #[test]
    fn test_read_request() {
        let request = build_read_request(0, reg::DRV_STATUS);

        assert_eq!(request[0], SYNC_BYTE);
        assert_eq!(request[1], 0); // address
        assert_eq!(request[2], reg::DRV_STATUS); // register (no write bit)
        assert_eq!(request[3], crc8(&request[..3])); // CRC
    }

    #[test]
    fn test_parse_read_response() {
        // Build a valid response
        let mut response = [0u8; 8];
        response[0] = SYNC_BYTE;
        response[1] = 0xFF; // master address
        response[2] = reg::DRV_STATUS;
        // Data: 0x12345678 big-endian
        response[3] = 0x12;
        response[4] = 0x34;
        response[5] = 0x56;
        response[6] = 0x78;
        response[7] = crc8(&response[..7]);

        let result = parse_read_response(&response);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0x12345678);

        // Test invalid sync
        let mut bad_sync = response;
        bad_sync[0] = 0x00;
        assert_eq!(
            parse_read_response(&bad_sync),
            Err(Tmc2209Error::InvalidSync)
        );

        // Test bad CRC
        let mut bad_crc = response;
        bad_crc[7] = 0x00;
        assert_eq!(
            parse_read_response(&bad_crc),
            Err(Tmc2209Error::CrcMismatch)
        );
    }

    #[test]
    fn test_set_current_datagram() {
        let config = Tmc2209Config::default();
        let driver = Tmc2209Driver::new(config);

        let datagram = driver.set_current_datagram(800, 400);

        assert_eq!(datagram[0], SYNC_BYTE);
        assert_eq!(datagram[2], reg::IHOLD_IRUN | 0x80); // write bit set
    }
}
