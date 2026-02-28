use esp_idf_svc::{
    bt::{
        BtDriver,
        ble::{
            gap::EspBleGap,
            gatt::{self, client::EspGattc},
        },
    },
    hal::{delay::FreeRtos, peripherals::Peripherals},
    log::EspLogger,
    nvs::EspDefaultNvsPartition,
};
use log::*;
use omnibench::{APP_ID, client::OmnibenchClient};
use std::sync::Arc;

pub fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    let bt = Arc::new(BtDriver::new(peripherals.modem, Some(nvs.clone()))?);

    let client = OmnibenchClient::new(
        Arc::new(EspBleGap::new(bt.clone())?),
        Arc::new(EspGattc::new(bt.clone())?),
    );

    info!("BLE Gap and Gattc initialized");

    let gap_client = client.clone();

    client.gap.subscribe(move |event| {
        gap_client.check_esp_status(gap_client.on_gap_event(event));
    })?;

    let gattc_client = client.clone();

    client.gattc.subscribe(move |(gatt_if, event)| {
        gattc_client.check_esp_status(gattc_client.on_gattc_event(gatt_if, event))
    })?;

    info!("BLE Gap and Gattc subscriptions initialized");

    client.gattc.register_app(APP_ID)?;

    info!("Gattc BTP app registered");

    gatt::set_local_mtu(500)?;

    info!("Gattc BTP app registered");

    client.wait_for_write_char_handle();
    let mut write_data = 0_u16;
    let mut indicate = true;

    loop {
        // Subscribe/unsubscribe to indications
        if write_data.is_multiple_of(10) {
            client.request_indicate(indicate)?;
            indicate = !indicate;
        }

        client.write_characterisitic(&write_data.to_le_bytes())?;

        info!("Wrote characteristic: {write_data}");

        write_data = write_data.wrapping_add(1);

        FreeRtos::delay_ms(5000);

        if write_data.is_multiple_of(30) {
            client.disconnect()?;
            FreeRtos::delay_ms(5000);
            client.connect()?;
        }
    }
}
