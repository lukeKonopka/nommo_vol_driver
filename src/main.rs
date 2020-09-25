use std::convert::TryFrom;
use std::fmt::{Display, Formatter, Result};
use subprocess::Exec;

const NOMMO_VENDOR_ID: u16 = 0x1532;
const NOMMO_PRODUCT_ID: u16 = 0x0517;
const VOL_STEP: u8 = 5;

#[derive(Debug, PartialEq)]
enum NommoMsg {
    VolUp,
    VolDown,
    EqValue(u8),
    Noop,
}

impl TryFrom<&[u8; 16]> for NommoMsg {
    type Error = String;

    fn try_from(
        other: &[u8; 16],
    ) -> std::result::Result<Self, <Self as std::convert::TryFrom<&[u8; 16]>>::Error> {
        match other {
            [1, 233, ..] => Ok(Self::VolUp),
            [1, 234, ..] => Ok(Self::VolDown),
            [5, 15, _, v, ..] => Ok(Self::EqValue(*v)),
            _ =>  Ok(Self::Noop),
        }
    }
}

#[derive(Debug, PartialEq)]
enum NommoVol {
    Value(u8),
    Muted,
}

impl NommoVol {
    fn inc(&self, step: u8) -> Self {
        match self {
            Self::Muted => Self::Value(step),
            Self::Value(val) => {
                if val + step > 100 {
                    Self::Value(100)
                } else {
                    Self::Value(val + step)
                }
            },
        }
    }

    fn dec(&self, step: u8) -> Self {
        match self {
            Self::Muted => Self::Muted,
            Self::Value(val) => {
                if val <= &step {
                    Self::Muted
                } else {
                    Self::Value(val - step)
                }
            }
        }
    }
}

impl TryFrom<String> for NommoVol {
    type Error = std::num::ParseIntError;
    fn try_from(
        other: String,
    ) -> std::result::Result<Self, <Self as std::convert::TryFrom<String>>::Error> {
        let number_string = &other[..other.len() - 1];
        let number_value = number_string.parse::<u8>()?;
        if number_value == 0 {
            Ok(Self::Muted)
        } else {
            Ok(Self::Value(number_value))
        }
    }
}

impl Display for NommoVol {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Self::Value(v) => write!(f, "{}%", v),
            Self::Muted => write!(f, "0%"),
        }
    }
}

use pulsectl::controllers::DeviceControl;
use pulsectl::controllers::SinkController;

fn handle_device(dev_handle: hidapi::HidDevice) {
    // get Pulse Audio default device
    let mut sink_controller = SinkController::create();
    let default_sink = sink_controller.get_default_device().expect("Cannot get PulseAudio default sink");

    let mut buff = [0 as u8; 16];
    loop {
        dev_handle.read(&mut buff).expect("Cannot read from device");
        let msg = NommoMsg::try_from(&buff).expect("Cannot convert data");

        match msg {
            NommoMsg::VolUp => {
                // TODO: add support for unmuting
                // TODO: add 100% cap
                sink_controller.increase_device_volume_by_percent(default_sink.index, VOL_STEP as f64 / 100.0);
            }
            NommoMsg::VolDown => {
                // TODO: add support for muting
                sink_controller.decrease_device_volume_by_percent(default_sink.index, VOL_STEP as f64 / 100.0);
            }
            _ => {}
        }
    }
}

fn debug_user_name() -> subprocess::Result<()> {
    let whoami_res = { Exec::shell("whoami") }.capture()?.stdout_str();

    println!("Current user: {}", whoami_res);
    Ok(())
}


fn main() {
    debug_user_name().expect("Cannot debug print username");

    // let default_sink_name = get_default_sink_name().expect("Cannot get default sink name");


    match hidapi::HidApi::new() {
        Ok(api) => {
            let device = api.open(NOMMO_VENDOR_ID, NOMMO_PRODUCT_ID);
            match device {
                Ok(handle) => handle_device(handle),
                Err(error) => {
                    eprintln!("Device error: {}", error);
                }
            }
        }
        Err(error) => {
            eprintln!("Error: {}", error);
        }
    }
}
