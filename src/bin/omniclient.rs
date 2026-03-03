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
        delay::Delay,
        gpio::PinDriver,
        i2c::{I2cConfig, I2cDriver},
        peripherals::Peripherals,
        units::FromValueType,
    },
    nvs::EspDefaultNvsPartition,
};
use log::*;
use omnibench::{
    APP_ID,
    client::{ConnectionStatus, OmnibenchClient},
    joystick::{QwiicJoy, QwiicJoyState},
    protocol::{ButtonEvent, RelayState},
};
use std::{
    cell::RefCell,
    sync::{Arc, Mutex},
    time::Duration,
};

const RED: NeoKey1x4Color = NeoKey1x4Color { r: 255, g: 0, b: 0 };
const BLUE: NeoKey1x4Color = NeoKey1x4Color { r: 0, g: 0, b: 255 };
const WHITE: NeoKey1x4Color = NeoKey1x4Color {
    r: 255,
    g: 255,
    b: 255,
};
const DIM_WHITE: NeoKey1x4Color = NeoKey1x4Color { r: 3, g: 3, b: 3 };
const ORANGE: NeoKey1x4Color = NeoKey1x4Color {
    r: 255,
    g: 128,
    b: 0,
};

const Y_ZERO: u16 = 495;
const Y_ZERO_CLIP_MIN: u16 = Y_ZERO - 5;
const Y_ZERO_CLIP_MAX: u16 = Y_ZERO + 5;

type NeoKeys<'bus> = NeoKey1x4<SeesawDriver<RefCellDevice<'bus, I2cDriver<'static>>, Delay>>;

fn button_color(pressed: bool, relay_on: bool) -> NeoKey1x4Color {
    if pressed {
        BLUE
    } else if relay_on {
        WHITE
    } else {
        DIM_WHITE
    }
}

fn update_strip(
    strip: &mut NeoKeys<'_>,
    nibble: u8,
    relay_state: RelayState,
    relay_offset: u8,
) -> anyhow::Result<()> {
    let colors = std::array::from_fn::<_, 4, _>(|i| {
        let i = i as u8;
        button_color((nibble >> i) & 1 == 0, relay_state.is_on(relay_offset + i))
    });
    strip
        .set_neopixel_colors(&colors)
        .and_then(|_| strip.sync_neopixel())
        .map_err(|e| anyhow::anyhow!("Neopixel error: {e:?}"))
}

fn set_uniform(strip: &mut NeoKeys<'_>, color: NeoKey1x4Color) -> anyhow::Result<()> {
    strip
        .set_neopixel_colors(&[color, color, color, color])
        .and_then(|_| strip.sync_neopixel())
        .map_err(|e| anyhow::anyhow!("Neopixel error: {e:?}"))
}

pub fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;

    // I2C
    let mut i2c_power = PinDriver::output(peripherals.pins.gpio7)?;
    i2c_power.set_low()?;

    info!("Initializing I2C and Seesaw");
    let (sda, scl) = (peripherals.pins.gpio3, peripherals.pins.gpio4);
    let config = I2cConfig::new().baudrate(400u32.kHz().into());
    let delay = Delay::new_default();
    let i2c = RefCell::new(I2cDriver::<'static>::new(
        peripherals.i2c0,
        sda,
        scl,
        &config,
    )?);
    i2c_power.set_high()?;
    std::thread::sleep(Duration::from_millis(50));
    let mut joy = QwiicJoy::new(RefCellDevice::new(&i2c));
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
    let client = OmnibenchClient::new(
        Arc::new(EspBleGap::new(bt.clone())?),
        Arc::new(EspGattc::new(bt.clone())?),
    );
    info!("BLE Gap and Gattc initialized");

    let relay_state = Arc::new(Mutex::new(RelayState::default()));

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

    let relay_state_cb = relay_state.clone();
    client.set_notify_callback(move |bytes| {
        if let Some(rs) = RelayState::from_bytes(bytes) {
            *relay_state_cb.lock().unwrap() = rs;
        }
    });

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
    let mut last_y_mapped: Option<i8> = None;

    loop {
        let status = client.status();
        let current_relay_state = *relay_state.lock().unwrap();

        let QwiicJoyState { y, .. } = joy.state().expect("Failed to get joystick state");
        let y_mapped = map_analog_to_i8(y);

        if Some(y_mapped) != last_y_mapped {
            info!("Joystick Y: {y} → {y_mapped}");
            last_y_mapped = Some(y_mapped);
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
                        client.write_characteristic(&ButtonEvent { relay: i }.to_bytes())?;
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

fn map_analog_to_i8(analog: u16) -> i8 {
    let normalized = (analog - Y_ZERO) as f32 / (Y_ZERO_CLIP_MAX - Y_ZERO_CLIP_MIN) as f32;
    (normalized * 127.0) as i8
}
