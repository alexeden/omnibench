use adafruit_seesaw::{
    SeesawDriver,
    devices::{NeoKey1x4, NeoKey1x4Color},
    prelude::*,
};
use embedded_hal_bus::i2c::RefCellDevice;
use esp_idf_svc::{
    bt::{
        BtDriver,
        ble::{
            gap::EspBleGap,
            gatt::{self, client::EspGattc},
        },
    },
    hal::{
        adc::{
            attenuation,
            oneshot::{
                AdcChannelDriver,
                config::{AdcChannelConfig, Calibration},
            },
        },
        delay::Delay,
        gpio::PinDriver,
        i2c::{I2cConfig, I2cDriver},
        peripherals::Peripherals,
        units::FromValueType,
    },
    nvs::EspDefaultNvsPartition,
    sys,
};
use log::*;
use omnibench::{
    APP_ID,
    board::map_mv_to_i8,
    client::{ConnectionStatus, OmnibenchClient},
    colors::{BLUE, DIM_WHITE, OFF, ORANGE, RED, WHITE, YELLOW},
    protocol::{ButtonEvent, ClientEvent, JoystickEvent, RelayState},
};
use std::{
    cell::RefCell,
    sync::{Arc, Mutex},
    time::Duration,
};

type NeoKeys<'bus> = NeoKey1x4<SeesawDriver<RefCellDevice<'bus, I2cDriver<'static>>, Delay>>;

pub fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    // Why'd we wake up?
    let wakeup_cause = unsafe { sys::esp_sleep_get_wakeup_cause() };
    match wakeup_cause {
        sys::esp_sleep_source_t_ESP_SLEEP_WAKEUP_EXT0 => {
            info!("Woke from joystick click");
        }
        sys::esp_sleep_source_t_ESP_SLEEP_WAKEUP_TIMER => {
            info!("Woke from timer");
        }
        _ => {
            info!("Fresh boot");
        }
    };

    let peripherals = Peripherals::take()?;

    let (adc, joy_pin, sw_pin) = omnibench::board_joy_adc!(peripherals);
    let mut joy_adc = AdcChannelDriver::new(
        &adc,
        joy_pin,
        &AdcChannelConfig {
            attenuation: attenuation::DB_12,
            calibration: {
                #[cfg(not(esp32s3))]
                let cal = Calibration::Line;
                #[cfg(esp32s3)]
                let cal = Calibration::Curve;
                cal
            },
            ..Default::default()
        },
    )?;

    // I2C
    info!("Initializing I2C and Seesaw");
    let (i2c_power, sda, scl) = omnibench::board_i2c_pins!(peripherals);
    let mut i2c_power = PinDriver::output(i2c_power)?;
    i2c_power.set_low()?;
    let config = I2cConfig::new().baudrate(400u32.kHz().into());
    let delay = Delay::new_default();
    let i2c = RefCell::new(I2cDriver::<'static>::new(
        peripherals.i2c1,
        sda,
        scl,
        &config,
    )?);
    i2c_power.set_high()?;
    std::thread::sleep(Duration::from_millis(50));
    let seesaw = SeesawDriver::new(delay, RefCellDevice::new(&i2c));
    let mut neokeys1 = NeoKey1x4::new(0x33, seesaw)
        .init()
        .expect("Failed to start NeoKey1x4");
    let seesaw = SeesawDriver::new(delay, RefCellDevice::new(&i2c));
    let mut neokeys2 = NeoKey1x4::new_with_default_addr(seesaw)
        .init()
        .expect("Failed to start NeoKey1x4");

    info!("Seesaw initialized");

    // Start blue — scanning is about to begin
    set_uniform(&mut neokeys1, BLUE)?;
    set_uniform(&mut neokeys2, BLUE)?;

    let nvs = EspDefaultNvsPartition::take()?;
    let bt = Arc::new(BtDriver::new(peripherals.modem, Some(nvs.clone()))?);
    let relay_state = Arc::new(Mutex::new(RelayState::default()));
    let relay_state_cb = relay_state.clone();
    let client = OmnibenchClient::new(
        Arc::new(EspBleGap::new(bt.clone())?),
        Arc::new(EspGattc::new(bt.clone())?),
        move |bytes| {
            if let Some(rs) = RelayState::from_bytes(bytes) {
                *relay_state_cb.lock().unwrap() = rs;
            }
        },
    );
    info!("BLE Gap and Gattc initialized");

    let gap_client = client.clone();
    client.gap.subscribe(move |event| {
        if let Err(e) = gap_client.on_gap_event(event) {
            warn!("Gap event error: {e:?}");
        }
    })?;

    let gattc_client = client.clone();
    client.gattc.subscribe(move |(gatt_if, event)| {
        if let Err(e) = gattc_client.on_gattc_event(gatt_if, event) {
            warn!("Gattc event error: {e:?}");
        }
    })?;

    info!("Subscriptions initialized; registering app and starting scan");
    client.gattc.register_app(APP_ID)?;
    gatt::set_local_mtu(500)?;

    // Main loop: update LEDs from connection status and relay state; handle
    // button presses for both relay toggling (connected) and rescan (disconnected).
    let mut last_status = None;
    let mut last_relay_state: Option<RelayState> = None;
    // Previous key bitmask for edge detection (0 = pressed, active low).
    // Start all-high (no keys pressed). Low nibble = strip 0, high nibble = strip
    // 1.
    let mut last_keys: u8 = 0xFF;

    loop {
        let status = client.status();
        let current_relay_state = *relay_state.lock().unwrap();
        if status == ConnectionStatus::Connected {
            let mut joy = map_mv_to_i8(adc.read(&mut joy_adc)?);
            if joy != 0 {
                set_uniform(&mut neokeys1, if joy < 0 { OFF } else { BLUE })?;
                set_uniform(&mut neokeys2, if joy < 0 { BLUE } else { OFF })?;
                loop {
                    joy = map_mv_to_i8(adc.read(&mut joy_adc)?);
                    info!("Joystick: {joy}");
                    client.write_characteristic(
                        &ClientEvent::Joystick(JoystickEvent { value: joy }).to_bytes(),
                    )?;
                    if joy == 0 {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                last_status = None; // force LED refresh on next iteration
            }
        }

        let k0 = neokeys1.keys().unwrap_or(last_keys & 0x0F) & 0x0F;
        let k1 = neokeys2.keys().unwrap_or(last_keys >> 4) & 0x0F;
        let keys = k0 | (k1 << 4);

        // Update LEDs when connection status, relay state, or key state changes.
        let leds_stale = Some(status) != last_status
            || (status == ConnectionStatus::Connected
                && (Some(current_relay_state) != last_relay_state || keys != last_keys));

        if leds_stale {
            info!("Connection status: {status:?}  Relay state: {current_relay_state:?}");
            match status {
                ConnectionStatus::Connected => {
                    update_strip(&mut neokeys1, k0, current_relay_state, 0)?;
                    update_strip(&mut neokeys2, k1, current_relay_state, 4)?;
                }
                _ => {
                    let color = match status {
                        ConnectionStatus::Scanning => BLUE,
                        ConnectionStatus::ScanFailed | ConnectionStatus::Disconnected => RED,
                        ConnectionStatus::Error => ORANGE,
                        ConnectionStatus::Connected => unreachable!(),
                    };
                    set_uniform(&mut neokeys1, color)?;
                    set_uniform(&mut neokeys2, color)?;
                }
            }
            last_status = Some(status);
            last_relay_state = Some(current_relay_state);
        }

        // Button handling — detect falling edge (bit 1 → 0 = key just pressed).
        match status {
            ConnectionStatus::Connected => {
                for i in 0..8u8 {
                    let bit = 1u8 << (i & 3) << (if i < 4 { 0 } else { 4 });
                    if (last_keys & bit) != 0 && (keys & bit) == 0 {
                        info!("Button {i} pressed — toggling relay {i}");
                        client.write_characteristic(
                            &ClientEvent::Button(ButtonEvent { relay: i }).to_bytes(),
                        )?;
                    }
                }
            }
            ConnectionStatus::ScanFailed
            | ConnectionStatus::Disconnected
            | ConnectionStatus::Error => {
                if last_keys == 0xFF && keys != 0xFF {
                    info!("Button pressed — restarting scan");
                    client.connect()?;
                }
            }
            ConnectionStatus::Scanning => {}
        }
        last_keys = keys;
    }
}

fn sleepy_time(joy_sw_pin: u32) {
    unsafe {
        // EXT0: single GPIO wakeup. Wake on LOW (joystick click pulls low)
        sys::esp_sleep_enable_ext0_wakeup(joy_sw_pin as i32, 0);
        sys::esp_deep_sleep_start(); // does not return
    }
}

fn set_uniform(strip: &mut NeoKeys<'_>, color: NeoKey1x4Color) -> anyhow::Result<()> {
    strip
        .set_neopixel_colors(&[color, color, color, color])
        .and_then(|_| strip.sync_neopixel())
        .map_err(|e| anyhow::anyhow!("Neopixel error: {e:?}"))
}

fn update_strip(
    strip: &mut NeoKeys<'_>,
    nibble: u8,
    relay_state: RelayState,
    relay_offset: u8,
) -> anyhow::Result<()> {
    strip
        .set_neopixel_colors(&std::array::from_fn::<_, 4, _>(|i| {
            let pressed = (nibble >> i) & 1 == 0;
            let on = relay_state.is_on(relay_offset + i as u8);
            if pressed {
                YELLOW
            } else if on {
                WHITE
            } else {
                DIM_WHITE
            }
        }))
        .and_then(|_| strip.sync_neopixel())
        .map_err(|e| anyhow::anyhow!("Neopixel error: {e:?}"))
}
