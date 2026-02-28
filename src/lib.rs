use esp_idf_svc::bt::BtUuid;
pub mod client;
pub mod server;

pub const APP_ID: u16 = 0;

// Our service UUID
pub(crate) const SERVICE_UUID: BtUuid = BtUuid::uuid128(0xad91b201734740479e173bed82d75f9d);

// bt_gatt_server name
pub(crate) const SERVER_NAME: &str = "ESP32";

/// Our "recv" characteristic - i.e. where clients can send data.
pub(crate) const RECV_CHARACTERISTIC_UUID: BtUuid =
    BtUuid::uuid128(0xb6fccb5087be44f3ae22f85485ea42c4);

/// Our "indicate" characteristic - i.e. where clients can receive data if they
/// subscribe to it
pub(crate) const IND_CHARACTERISTIC_UUID: BtUuid =
    BtUuid::uuid128(0x503de214868246c4828fd59144da41be);

// Client Characteristic Configuration UUID
pub(crate) const IND_DESCRIPTOR_UUID: BtUuid = BtUuid::uuid16(0x2902);
