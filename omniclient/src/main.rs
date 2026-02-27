use esp_idf_svc::{
    bt::{
        BtDriver,
        ble::{gap::EspBleGap, gatt::server::EspGatts},
    },
    hal::peripherals::Peripherals,
    nvs::EspDefaultNvsPartition,
};
use log::*;
use omnibench::{APP_ID, server::OmnibenchServer};
use std::sync::Arc;

fn main() -> anyhow::Result<()> {
    esp_idf_svc::hal::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    let bt = Arc::new(BtDriver::new(peripherals.modem, Some(nvs.clone()))?);

    let server = OmnibenchServer::new(
        Arc::new(EspBleGap::new(bt.clone())?),
        Arc::new(EspGatts::new(bt.clone())?),
    );

    info!("BLE Gap and Gatts initialized");

    let gap_server = server.clone();

    server.gap.subscribe(move |event| {
        gap_server.check_esp_status(gap_server.on_gap_event(event));
    })?;

    let gatts_server = server.clone();

    server.gatts.subscribe(move |(gatt_if, event)| {
        gatts_server.check_esp_status(gatts_server.on_gatts_event(gatt_if, event))
    })?;

    info!("BLE Gap and Gatts subscriptions initialized");

    server.gatts.register_app(APP_ID)?;

    info!("Gatts BTP app registered");

    let mut ind_data = 0_u16;

    loop {
        server.indicate(&ind_data.to_le_bytes())?;
        info!("Broadcasted indication: {ind_data}");

        ind_data = ind_data.wrapping_add(1);

        std::thread::sleep(std::time::Duration::from_secs(10));
    }
}
