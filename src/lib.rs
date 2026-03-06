use esp_idf_svc::bt::BtUuid;
pub mod board;
pub mod client;
pub mod joystick;
pub mod protocol;
pub mod server;
pub mod stepper;

pub const APP_ID: u16 = 0;

/// Our service UUID
///
/// UUID: ad91b201-7347-4047-9e17-3bed82d75f9d
pub(crate) const SERVICE_UUID: BtUuid = BtUuid::uuid128(0xad91b201734740479e173bed82d75f9d);

/// Device name
pub(crate) const SERVER_NAME: &str = "Omnibench";

/// Our "recv" characteristic - i.e. where clients can send data
///
/// UUID: b6fccb50-87be-44f3-ae22-f85485ea42c4
pub(crate) const RECV_CHARACTERISTIC_UUID: BtUuid =
    BtUuid::uuid128(0xb6fccb5087be44f3ae22f85485ea42c4);

/// Our "notify" characteristic - i.e. where clients can receive data if they
/// subscribe to it
///
/// UUID: 503de214-8682-46c4-828f-d59144da41be
pub(crate) const NOTIFY_CHARACTERISTIC_UUID: BtUuid =
    BtUuid::uuid128(0x503de214868246c4828fd59144da41be);

/// Client Characteristic Configuration UUID
pub(crate) const NOTIFY_DESCRIPTOR_UUID: BtUuid = BtUuid::uuid16(0x2902);
