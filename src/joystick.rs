use embedded_hal::i2c::I2c;

pub const QWIIC_JOY_DEFAULT_ADDR: u8 = 0x20;

/// Default I2C Address
pub(crate) const DEFAULT_ADDR_REG: u8 = 0x00;

/// Firmware Version
pub(crate) const _FW_VERS_H_REG: u8 = 0x01;
pub(crate) const _FW_VERS_L_REG: u8 = 0x02;

/// Horizontal Position (MSB First)
pub(crate) const H_POS_H_REG: u8 = 0x03;
pub(crate) const _H_POS_L_REG: u8 = 0x04;

/// Vertical Position (MSB First)
pub(crate) const V_POS_H_REG: u8 = 0x05;
pub(crate) const _V_POS_L_REG: u8 = 0x06;

/// Button Position
pub(crate) const BUTTON_REG: u8 = 0x07;

/// Button Status: Indicates if button was pressed since last read of button
/// state. Clears after read.
pub(crate) const BUTTON_STATUS_REG: u8 = 0x08;

/// Lock Register for I2C Address Change
pub(crate) const _LOCK_REG: u8 = 0x09;

/// Current I2C Slave Address. Can only be changed once Lock Register is set to
/// 0x13, then it clears the Lock Register.
pub(crate) const _CURRENT_ADDR_REG: u8 = 0x0A;

#[derive(Debug)]
pub enum Error<E> {
    /// I2C bus error
    I2c(E),
}

#[derive(Debug)]
pub struct QwiicJoy<I>(I);

#[derive(Copy, Clone, Debug)]
pub struct QwiicJoyState {
    pub x: u16,
    pub y: u16,
    pub btn: bool,
}

impl From<[u8; 5]> for QwiicJoyState {
    /// Swaps X and Y which makes more sense looking at the hardware
    fn from(bytes: [u8; 5]) -> Self {
        Self {
            y: (((bytes[0] as u16) << 8) | (bytes[1] as u16)) >> 6_u16,
            x: (((bytes[2] as u16) << 8) | (bytes[3] as u16)) >> 6_u16,
            btn: bytes[4] == 0,
        }
    }
}

impl<I: I2c> QwiicJoy<I> {
    pub fn new(i2c: I) -> Self {
        Self(i2c)
    }

    pub fn btn(&mut self) -> Result<bool, I::Error> {
        self.read_u8(BUTTON_REG).map(|byte| byte == 1)
    }

    pub fn btn_check(&mut self) -> Result<bool, I::Error> {
        self.read_u8(BUTTON_STATUS_REG).map(|byte| byte == 1)
    }

    pub fn default_address(&mut self) -> Result<u8, I::Error> {
        self.read_u8(DEFAULT_ADDR_REG)
    }

    pub fn x(&mut self) -> Result<u16, I::Error> {
        self.read_u16(H_POS_H_REG).map(|h_pos| h_pos >> 6)
    }

    pub fn y(&mut self) -> Result<u16, I::Error> {
        self.read_u16(V_POS_H_REG).map(|h_pos| h_pos >> 6)
    }

    pub fn state(&mut self) -> Result<QwiicJoyState, I::Error> {
        let mut bytes: [u8; 5] = [0; 5];

        self.0
            .write_read(QWIIC_JOY_DEFAULT_ADDR, &[H_POS_H_REG], &mut bytes)
            .map(|_| bytes.into())
    }

    fn read_u8(&mut self, reg: u8) -> Result<u8, I::Error> {
        let mut byte: [u8; 1] = [0; 1];

        self.0
            .write_read(QWIIC_JOY_DEFAULT_ADDR, &[reg], &mut byte)
            .map(|_| byte[0])
    }

    fn read_u16(&mut self, reg: u8) -> Result<u16, I::Error> {
        let mut bytes: [u8; 2] = [0; 2];

        self.0
            .write_read(QWIIC_JOY_DEFAULT_ADDR, &[reg], &mut bytes)
            .map(|_| (bytes[0] as u16) << 8 | bytes[1] as u16)
    }
}
