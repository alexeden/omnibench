use esp_idf_svc::bt::BtUuid;
use std::time::Duration;
pub mod board;
pub mod client;
mod freq_gen;
pub mod protocol;
pub mod server;
pub mod stepper;

pub const SLEEP_TIMEOUT: Duration = Duration::from_secs(5);

pub const APP_ID: u16 = 0;

/// Our service UUID
/// UUID: ad91b201-7347-4047-9e17-3bed82d75f9d
pub(crate) const SERVICE_UUID: BtUuid = BtUuid::uuid128(0xad91b201734740479e173bed82d75f9d);

/// Device name
pub(crate) const SERVER_NAME: &str = "Omnibench";

/// "recv" characteristic - i.e. where clients send data
/// UUID: b6fccb50-87be-44f3-ae22-f85485ea42c4
pub(crate) const RECV_CHARACTERISTIC_UUID: BtUuid =
    BtUuid::uuid128(0xb6fccb5087be44f3ae22f85485ea42c4);

/// "notify" characteristic - i.e. where clients receive data if they subscribe
/// UUID: 503de214-8682-46c4-828f-d59144da41be
pub(crate) const NOTIFY_CHARACTERISTIC_UUID: BtUuid =
    BtUuid::uuid128(0x503de214868246c4828fd59144da41be);

/// Client Characteristic Configuration UUID
pub(crate) const NOTIFY_DESCRIPTOR_UUID: BtUuid = BtUuid::uuid16(0x2902);

pub mod colors {
    use adafruit_seesaw::devices::NeoKey1x4Color;

    pub const RED: NeoKey1x4Color = NeoKey1x4Color { r: 255, g: 0, b: 0 };
    pub const BLUE: NeoKey1x4Color = NeoKey1x4Color { r: 0, g: 0, b: 255 };
    pub const WHITE: NeoKey1x4Color = NeoKey1x4Color {
        r: 255,
        g: 255,
        b: 255,
    };
    pub const DIM_WHITE: NeoKey1x4Color = NeoKey1x4Color { r: 3, g: 3, b: 3 };
    pub const ORANGE: NeoKey1x4Color = NeoKey1x4Color {
        r: 255,
        g: 128,
        b: 0,
    };
    pub const YELLOW: NeoKey1x4Color = NeoKey1x4Color {
        r: 255,
        g: 255,
        b: 0,
    };
    pub const OFF: NeoKey1x4Color = NeoKey1x4Color { r: 0, g: 0, b: 0 };
}
