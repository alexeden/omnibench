use crate::{
    APP_ID, NOTIFY_CHARACTERISTIC_UUID, NOTIFY_DESCRIPTOR_UUID, RECV_CHARACTERISTIC_UUID,
    SERVER_NAME, SERVICE_UUID,
};
use esp_idf_svc::{
    bt::{
        BdAddr, Ble, BtDriver, BtStatus,
        ble::{
            gap::{
                AdvertisingDataType, BleGapEvent, EspBleGap, GapSearchEvent, GapSearchResult,
                ScanParams,
            },
            gatt::{
                GattInterface, GattStatus, Handle, Property,
                client::{
                    CharacteristicElement, ConnectionId, DbAttrType, DescriptorElement, EspGattc,
                    GattAuthReq, GattCreateConnParams, GattWriteType, GattcEvent,
                },
            },
        },
    },
    sys::{ESP_FAIL, EspError},
};
use log::*;
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum ConnectionStatus {
    /// Actively scanning for the server (blue)
    #[default]
    Scanning,
    /// Write characteristic discovered; ready to communicate (white)
    Connected,
    /// Was connected, now disconnected (red)
    Disconnected,
    /// Scan completed without finding the server (red)
    ScanFailed,
    /// An unexpected BT/GATT error occurred (orange)
    Error,
}

type ExBtDriver = BtDriver<'static, Ble>;
type ExEspBleGap = Arc<EspBleGap<'static, Ble, Arc<ExBtDriver>>>;
type ExEspGattc = Arc<EspGattc<'static, Ble, Arc<ExBtDriver>>>;
type NotifyHandler = Arc<dyn Fn(&[u8]) + Send + Sync>;

#[derive(Default)]
struct State {
    conn_id: Option<ConnectionId>,
    connected: bool,
    gattc_if: Option<GattInterface>,
    ind_char_handle: Option<Handle>,
    ind_descr_handle: Option<Handle>,
    remote_addr: Option<BdAddr>,
    service_start_end_handle: Option<(Handle, Handle)>,
    status: ConnectionStatus,
    write_char_handle: Option<Handle>,
}

#[derive(Clone)]
pub struct OmnibenchClient {
    pub gap: ExEspBleGap,
    pub gattc: ExEspGattc,
    notify_callback: NotifyHandler,
    state: Arc<Mutex<State>>,
}

impl OmnibenchClient {
    pub fn new(
        gap: ExEspBleGap,
        gattc: ExEspGattc,
        notify_callback: impl Fn(&[u8]) + Send + Sync + 'static,
    ) -> Self {
        Self {
            gap,
            gattc,
            notify_callback: Arc::new(notify_callback),
            state: Arc::new(Mutex::new(Default::default())),
        }
    }

    /// Connect to the bt_gatt_server.
    ///
    /// This sets the scan params with triggers the event
    /// `BleGapEvent::ScanParameterConfigured` where the gap callback will start
    /// scanning. Scanning must happen before a connect can be made in
    /// `BleGapEvent::ScanResult`.
    pub fn connect(&self) -> Result<(), EspError> {
        if !self.state.lock().unwrap().connected {
            let scan_params = ScanParams {
                scan_interval: 0x50,
                ..Default::default()
            };
            self.gap.set_scan_params(&scan_params)?;
        }
        Ok(())
    }

    /// Returns the current connection status, suitable for driving UI feedback.
    pub fn status(&self) -> ConnectionStatus {
        self.state.lock().unwrap().status
    }

    // Write some data to the write characteristic.
    pub fn write_characteristic(&self, char_value: &[u8]) -> Result<(), EspError> {
        let state = self.state.lock().unwrap();

        let Some(gattc_if) = state.gattc_if else {
            return Ok(());
        };
        let Some(conn_id) = state.conn_id else {
            return Ok(());
        };

        if let Some(write_char_handle) = state.write_char_handle {
            self.gattc.write_characteristic(
                gattc_if,
                conn_id,
                write_char_handle,
                char_value,
                GattWriteType::RequireResponse,
                GattAuthReq::None,
            )?;
        }

        Ok(())
    }

    /// The main event handler for the GAP events
    pub fn on_gap_event(&self, event: BleGapEvent) -> Result<(), EspError> {
        match event {
            BleGapEvent::ScanParameterConfigured(status) => {
                info!("BleGapEvent::ScanParameterConfigured: {status:?}");
                self.check_bt_status(status)?;
                self.gap.start_scanning(10)?;
            }
            BleGapEvent::ScanStarted(status) => {
                info!("BleGapEvent::ScanStarted: {status:?}");
                self.check_bt_status(status)?;
                self.state.lock().unwrap().status = ConnectionStatus::Scanning;
            }
            BleGapEvent::ScanResult(GapSearchEvent::InquiryComplete(_)) => {
                let mut state = self.state.lock().unwrap();
                if !state.connected {
                    info!("Scan completed, no server {SERVER_NAME} found");
                    state.status = ConnectionStatus::ScanFailed;
                }
            }
            BleGapEvent::ScanResult(GapSearchEvent::InquiryResult(GapSearchResult {
                bda,
                ble_addr_type,
                rssi: _,
                ble_adv,
                ..
            })) => {
                let name = ble_adv
                    .and_then(|ble_adv| {
                        self.gap
                            .resolve_adv_data_by_type(ble_adv, AdvertisingDataType::NameCmpl)
                    })
                    .map(|n| std::str::from_utf8(n))
                    .transpose()
                    .ok()
                    .flatten();
                // info!("BleGapEvent::ScanResult(GapSearchEvent::InquiryResult): {n:?}");
                if let Some(name) = name.filter(|n| *n == SERVER_NAME) {
                    info!("!!! Device found: {name:?}");
                    let mut state = self.state.lock().unwrap();
                    if !state.connected {
                        state.connected = true;
                        info!("!!! Connect to remote {bda}");
                        self.gap.stop_scanning()?;
                        let conn_params = GattCreateConnParams::new(bda, ble_addr_type);
                        self.gattc.enh_open(state.gattc_if.unwrap(), &conn_params)?;
                    }
                }
            }
            BleGapEvent::ScanStopped(status) => {
                info!("BleGapEvent::ScanStopped: {status:?}");
                self.check_bt_status(status)?;
            }
            evt => {
                info!("BleGapEvent: {evt:?}");
            }
        }

        Ok(())
    }

    /// The main event handler for the GATTC events
    pub fn on_gattc_event(
        &self,
        gattc_if: GattInterface,
        event: GattcEvent,
    ) -> Result<(), EspError> {
        // Check if the event has an error status
        if let Some(status) = status_from_gattc_event(&event)
            && !matches!(status, GattStatus::Ok)
        {
            error!("ERROR GattcEvent: {event:?} got status: {status:?}");
            self.state.lock().unwrap().status = ConnectionStatus::Error;
            return Err(EspError::from_infallible::<ESP_FAIL>());
        }

        match event {
            GattcEvent::ClientRegistered { app_id, .. } if APP_ID == app_id => {
                info!("GattcEvent::ClientRegistered: will connect...");
                self.state.lock().unwrap().gattc_if = Some(gattc_if);
                self.connect()?;
            }
            GattcEvent::Connected { conn_id, addr, .. } => {
                info!("GattcEvent::Connected: connected to {addr}");
                let mut state = self.state.lock().unwrap();
                state.conn_id = Some(conn_id);
                state.remote_addr = Some(addr);
                self.gattc.mtu_req(gattc_if, conn_id)?;
            }
            GattcEvent::DiscoveryCompleted { conn_id, .. } => {
                info!("GattcEvent::DiscoveryCompleted: conn_id {conn_id}");
                self.gattc
                    .search_service(gattc_if, conn_id, Some(&SERVICE_UUID))?;
            }
            GattcEvent::SearchResult {
                start_handle,
                end_handle,
                srvc_id,
                ..
            } if srvc_id.uuid == SERVICE_UUID => {
                info!("GattcEvent::SearchResult: service found",);
                self.state.lock().unwrap().service_start_end_handle =
                    Some((start_handle, end_handle));
            }
            GattcEvent::SearchComplete {
                conn_id,
                searched_service_source,
                ..
            } => {
                info!(
                    "GattcEvent::SearchComplete: searched_service_source \
                     {searched_service_source:?}"
                );
                let mut state = self.state.lock().unwrap();

                if let Some((start_handle, end_handle)) = state.service_start_end_handle {
                    let char_count = self
                        .gattc
                        .get_attr_count(
                            gattc_if,
                            conn_id,
                            DbAttrType::Characteristic {
                                start_handle,
                                end_handle,
                            },
                        )
                        .map_err(|status| {
                            error!("Get attr count error for service {status:?}");
                            EspError::from_infallible::<ESP_FAIL>()
                        })?;

                    info!("Found {char_count} characteristics");

                    if char_count == 0 {
                        error!("No characteristics found");
                        return Ok(());
                    }
                    // Get the indicator characteristic handle and register for notification
                    let mut chars = [CharacteristicElement::new(); 1];

                    match self.gattc.get_characteristic_by_uuid(
                        gattc_if,
                        conn_id,
                        start_handle,
                        end_handle,
                        &NOTIFY_CHARACTERISTIC_UUID,
                        &mut chars,
                    ) {
                        Ok(_) => {
                            if let Some(ind_char_elem) = chars.first() {
                                if ind_char_elem.properties().contains(Property::Notify) {
                                    if let Some(remote_addr) = state.remote_addr {
                                        state.ind_char_handle = Some(ind_char_elem.handle());
                                        self.gattc.register_for_notify(
                                            gattc_if,
                                            &remote_addr,
                                            ind_char_elem.handle(),
                                        )?;
                                    }
                                } else {
                                    error!("Notify characteristic does not have property Notify");
                                }
                            }
                        }
                        Err(status) => {
                            error!("Get notify characteristic error {status:?}");
                        }
                    };

                    // Get the write characteristic handle and start sending data to the server
                    match self.gattc.get_characteristic_by_uuid(
                        gattc_if,
                        conn_id,
                        start_handle,
                        end_handle,
                        &RECV_CHARACTERISTIC_UUID,
                        &mut chars,
                    ) {
                        Ok(char_count) if char_count > 0 => {
                            if let Some(write_char_elem) = chars.first() {
                                if write_char_elem.properties().contains(Property::Write) {
                                    state.write_char_handle = Some(write_char_elem.handle());
                                    state.status = ConnectionStatus::Connected;
                                } else {
                                    error!("Write characteristic does not have property Write");
                                }
                            }
                        }
                        Ok(_) => {
                            error!("No write characteristic found");
                        }
                        Err(status) => {
                            error!("Get write characteristic error {status:?}");
                        }
                    };
                };
            }
            GattcEvent::RegisterNotify { handle, .. } => {
                info!("GattcEvent::RegisterNotify: notification registered successfully");

                // Extract conn_id while holding the lock briefly, then release it
                // before any further BLE API calls.
                let conn_id = match self.state.lock().unwrap().conn_id {
                    Some(id) => id,
                    None => return Ok(()),
                };

                let count = self
                    .gattc
                    .get_attr_count(gattc_if, conn_id, DbAttrType::Descriptor { handle })
                    .map_err(|status| {
                        error!("Get attr count for ind char error {status:?}");
                        EspError::from_infallible::<ESP_FAIL>()
                    })?;

                if count == 0 {
                    error!("No ind char descriptors found");
                    return Ok(());
                }

                let mut descrs = [DescriptorElement::new(); 1];
                let n = match self.gattc.get_descriptor_by_char_handle(
                    gattc_if,
                    conn_id,
                    handle,
                    &NOTIFY_DESCRIPTOR_UUID,
                    &mut descrs,
                ) {
                    Ok(n) => n,
                    Err(status) => {
                        error!("Get ind char descriptors error {status:?}");
                        return Ok(());
                    }
                };

                if n == 0 {
                    error!("No ind descriptor found");
                    return Ok(());
                }

                let Some(descr) = descrs
                    .first()
                    .filter(|d| d.uuid() == NOTIFY_DESCRIPTOR_UUID)
                else {
                    error!("No ind descriptor found");
                    return Ok(());
                };

                let descr_handle = descr.handle();
                self.state.lock().unwrap().ind_descr_handle = Some(descr_handle);

                // Write CCCD = 0x0001 to enable notifications from the server.
                info!("Enabling notifications");
                self.gattc.write_descriptor(
                    gattc_if,
                    conn_id,
                    descr_handle,
                    &1u16.to_le_bytes(),
                    GattWriteType::RequireResponse,
                    GattAuthReq::None,
                )?;
            }
            GattcEvent::Notify { handle, value, .. } => {
                // Clone the Arc while holding the lock (cheap), then release
                // before invoking so the handler can safely call back into the client.
                let handler = {
                    let state = self.state.lock().unwrap();
                    if Some(handle) == state.ind_char_handle {
                        Some(self.notify_callback.clone())
                    } else {
                        None
                    }
                };
                if let Some(handler) = handler {
                    handler(value);
                }
            }
            GattcEvent::Disconnected { addr, reason, .. } => {
                let mut state = self.state.lock().unwrap();
                state.connected = false;
                state.remote_addr = None;
                state.conn_id = None;
                state.service_start_end_handle = None;
                state.ind_char_handle = None;
                state.ind_descr_handle = None;
                state.write_char_handle = None;
                state.status = ConnectionStatus::Disconnected;
                info!("GattcEvent::Disconnected: remote {addr}, reason {reason:?}");
            }
            _evt => {
                // info!("______ GattcEvent: {evt:?}");
            }
        }
        Ok(())
    }

    fn check_bt_status(&self, status: BtStatus) -> Result<(), EspError> {
        if !matches!(status, BtStatus::Success) {
            warn!("!!! ERROR STATUS !!!: {status:?}");
            self.state.lock().unwrap().status = ConnectionStatus::Error;
            Err(EspError::from_infallible::<ESP_FAIL>())
        } else {
            Ok(())
        }
    }
}

fn status_from_gattc_event(event: &GattcEvent) -> Option<GattStatus> {
    match event {
        GattcEvent::AddressList { status, .. }
        | GattcEvent::ClientRegistered { status, .. }
        | GattcEvent::Close { status, .. }
        | GattcEvent::DiscoveryCompleted { status, .. }
        | GattcEvent::ExecWrite { status, .. }
        | GattcEvent::Mtu { status, .. }
        | GattcEvent::Open { status, .. }
        | GattcEvent::PrepareWrite { status, .. }
        | GattcEvent::QueueFull { status, .. }
        | GattcEvent::ReadCharacteristic { status, .. }
        | GattcEvent::ReadDescriptor { status, .. }
        | GattcEvent::ReadMultipleChar { status, .. }
        | GattcEvent::ReadMultipleVarChar { status, .. }
        | GattcEvent::RegisterNotify { status, .. }
        | GattcEvent::SearchComplete { status, .. }
        | GattcEvent::SetAssociation { status, .. }
        | GattcEvent::UnregisterNotify { status, .. }
        | GattcEvent::WriteCharacteristic { status, .. }
        | GattcEvent::WriteDescriptor { status, .. } => Some(*status),
        _ => None,
    }
}
