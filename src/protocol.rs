/// State of the 4 relays, sent by the server to connected clients.
///
/// Serializes to a single byte where bit N corresponds to relay N:
/// `1` = on, `0` = off.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RelayState(u8);

impl RelayState {
    pub fn is_on(self, relay: u8) -> bool {
        (self.0 >> relay) & 1 != 0
    }

    pub fn set(self, relay: u8, on: bool) -> Self {
        if on {
            Self(self.0 | (1 << relay))
        } else {
            Self(self.0 & !(1 << relay))
        }
    }

    pub fn toggle(self, relay: u8) -> Self {
        Self(self.0 ^ (1 << relay))
    }

    pub fn to_bytes(self) -> [u8; 1] {
        [self.0]
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        bytes.first().map(|&b| Self(b))
    }
}

impl From<RelayState> for [u8; 1] {
    fn from(state: RelayState) -> Self {
        [state.0]
    }
}

/// A button press event sent from the remote to the server.
///
/// Each button corresponds directly to a relay; pressing it requests that the
/// server toggle that relay.  Serializes to a single byte (the relay index).
#[derive(Clone, Copy, Debug)]
pub struct ButtonEvent {
    pub relay: u8,
}

impl ButtonEvent {
    pub fn to_bytes(self) -> [u8; 1] {
        [self.relay]
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        bytes
            .first()
            .copied()
            .filter(|&b| b < 8)
            .map(|relay| Self { relay })
    }
}

impl From<ButtonEvent> for [u8; 1] {
    fn from(event: ButtonEvent) -> Self {
        [event.relay]
    }
}

/// A joystick axis value sent from the remote to the server.
///
/// Serializes to a signed byte in the range -127..=127.
#[derive(Clone, Copy, Debug)]
pub struct JoystickEvent {
    pub value: i8,
}

/// A tagged client→server message that multiplexes over the recv
/// characteristic.
///
/// Wire format: `[tag, payload]`
///   - `[0x01, relay]`  — button press (relay index 0–7)
///   - `[0x02, value]`  — joystick axis (i8 cast to u8)
#[derive(Clone, Copy, Debug)]
pub enum ClientEvent {
    Button(ButtonEvent),
    Joystick(JoystickEvent),
}

impl ClientEvent {
    pub fn to_bytes(self) -> [u8; 2] {
        match self {
            ClientEvent::Button(e) => [0x01, e.relay],
            ClientEvent::Joystick(e) => [0x02, e.value as u8],
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let (&tag, rest) = bytes.split_first()?;
        let &payload = rest.first()?;
        match tag {
            0x01 if payload < 8 => Some(ClientEvent::Button(ButtonEvent { relay: payload })),
            0x02 => Some(ClientEvent::Joystick(JoystickEvent {
                value: payload as i8,
            })),
            _ => None,
        }
    }
}
