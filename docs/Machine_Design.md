# Machine Design Guidelines

This document covers mechanical and electrical design considerations for building watch cleaning machines compatible with Isochron firmware.

## Overview

A watch cleaning machine moves a basket through multiple jars containing cleaning solutions, rinses, and a drying chamber. The design must prioritize:

- Gentle, smooth motion to avoid damaging delicate watch parts
- Safety around volatile solvents
- Reliability for repeated use

## Mechanical Architecture

### Basket Drive

The basket rotates to agitate watch parts in solution. Key considerations:

**Thrust Bearings**
- The basket shaft MUST use thrust bearings to handle axial load
- The drive motor should never carry axial load from the basket weight
- Use a dedicated thrust bearing at the basket mount point

**Belt Drive**
- Use GT2 timing belt (6mm width recommended) between motor and basket shaft
- Belt provides vibration isolation for smoother motion
- Recommended gear ratio: 3:1 to 4:1 reduction
- Belt tensioning should be adjustable

**Motor Selection**
- NEMA 14 or NEMA 17 bipolar stepper motors work well
- Target speed range: 60-150 RPM at the basket
- With 3:1 reduction and 16 microsteps: motor runs at 180-450 RPM

### Shaft and Bearings

```
        ───────┬───────  ← Frame/top plate
               │
        ═══════╪═══════  ← Thrust bearing (handles hanging basket weight)
               │
        ═══════╪═══════  ← Radial bearing
               │
        ┌──────┴──────┐
        │ Driven      │  ← GT2 pulley
        │ Pulley      │
        └──────┬──────┘
               │
        ═══════╪═══════  ← Radial bearing
               │
               │
        ┌──────┴──────┐
        │   Basket    │  ← Hangs down into jar
        └─────────────┘
```

- Use sealed bearings rated for the environment
- Stainless steel shafts resist solvent exposure
- All rotating components should be above the solvent level

### Motor Mounting

- Mount motors ABOVE the maximum solvent level
- Ensure adequate ventilation around motor and driver
- Use flexible coupling or belt to isolate motor vibration
- TMC2209 drivers in StealthChop mode minimize motor noise

## Jar Positions

### Manual Machines

For machines where the operator moves the basket between jars:

- Jars should be easily accessible
- Clear labeling for jar sequence (Clean → Rinse 1 → Rinse 2 → Dry)
- Consider a jar holder/tray for consistent positioning

### Automated Machines

For machines with motorized positioning:

**Tower/Carousel Design**
- Jars arranged in a circle
- Single rotation motor moves between positions
- Use position sensor or homing switch for accurate positioning

**Linear Design**
- Jars arranged in a line
- Linear actuator or belt-driven carriage
- End stops for homing

**Lift Mechanism**
- Leadscrew or belt-driven vertical motion
- Limit switches at top and bottom
- Sufficient travel to clear jar rims

## Drying Chamber

The drying stage requires:

- Heater element (50W-100W typical)
- Temperature sensor (NTC 100K thermistor)
- Adequate airflow for moisture removal
- Maximum temperature: 55°C (firmware enforced)

**Heater Safety**
- Use thermal fuse as backup protection
- Heater should not contact basket or parts
- Ensure heater is disabled when lid is open (if equipped)

## Solvent Safety

Watch cleaning typically uses volatile solvents. Design considerations:

**Ventilation**
- Operate in well-ventilated area
- Consider exhaust fan for enclosed machines
- Never use near open flames or sparks

**Material Compatibility**
- Avoid plastics that dissolve in cleaning solvents
- Stainless steel, glass, and PTFE are generally safe
- Test all materials with your specific solvents

**Electrical Safety**
- All electronics should be above/away from solvent vapors
- Use sealed connectors where possible
- Ground all metal components

**Spill Containment**
- Design for easy cleanup of spills
- Electronics should survive minor splashes
- Consider drip trays under jars

## Electrical Integration

### Stepper Drivers

The firmware is designed for TMC2209 drivers:

- UART communication for configuration
- StealthChop for silent operation
- StallGuard for stall detection (optional)

**Wiring**
- Keep motor wires short to reduce EMI
- Use shielded cable for long runs
- Separate high-current motor wiring from signal wiring

### Sensors

**Temperature (NTC 100K)**
- Mount thermistor close to heater for accurate readings
- Use heat-resistant wiring
- 4.7K pullup resistor (typically on controller board)

**End Stops (if used)**
- Mechanical switches or optical sensors
- Use NC (normally closed) configuration for fail-safe
- Mount securely to prevent false triggers

### Display

V0 Display connection:
- 4-wire connection: 5V, GND, TX, RX
- UART at 115200 baud
- Keep cable under 1m for reliable communication

## Example Configurations

### Simple Manual Machine

- Single jar, manual basket movement
- Spin motor only
- Optional drying heater
- Minimal automation, maximum simplicity

### Semi-Automated

- Multiple jars in fixed positions
- Motorized lift for basket
- Operator rotates/positions between jars
- Programmable spin cycles per jar

### Fully Automated

- Motorized positioning (tower or linear)
- Motorized lift
- Automatic jar-to-jar movement
- Run complete program unattended

## Bill of Materials (Reference)

| Component | Specification | Notes |
|-----------|--------------|-------|
| Controller | BTT SKR Pico | RP2040 + TMC2209 drivers |
| Display | Voron V0 Display | OLED + rotary encoder |
| Spin Motor | NEMA 14/17 stepper | 1.8° step angle |
| Lift Motor | NEMA 17 stepper | If automated |
| Position Motor | NEMA 17 stepper | If automated |
| Heater | 50-100W cartridge | 24V recommended |
| Thermistor | NTC 100K | Glass bead type |
| Thrust Bearing | 8mm ID minimum | Match shaft size |
| Timing Belt | GT2 6mm | Closed loop, size to fit |
| Pulleys | GT2 16-20 tooth | Motor and driven |

## References

- [SKR Pico Documentation](https://github.com/bigtreetech/SKR-Pico)
- [TMC2209 Datasheet](https://www.trinamic.com/products/integrated-circuits/details/tmc2209-la/)
- [Voron V0 Display](https://github.com/VoronDesign/Voron-Hardware/tree/master/V0_Display)
