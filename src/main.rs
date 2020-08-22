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
            [1, 0, ..] => Ok(Self::Noop),
            _ => Err(format!("Unknown msg: {:?}", other)),
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

fn get_default_sink_name() -> subprocess::Result<String> {
    let pactl_info_default_sink = {
        Exec::shell("pactl info")
            | Exec::cmd("grep").arg("Default Sink")
            | Exec::cmd("tail").arg("--bytes=+15")
    }
    .capture()?
    .stdout_str();

    let name: String = pactl_info_default_sink.split_whitespace().collect();

    match name.len() {
        0 => Err(subprocess::PopenError::LogicError(
            "No default sink detected",
        )),
        _ => Ok(name),
    }
}

fn get_sink_volume(sink_name: &str) -> subprocess::Result<NommoVol> {
    let pactl_sink_volume = {
        Exec::shell("pactl list sinks")
            | Exec::cmd("grep")
                .arg("-A 10")
                .arg(format!("Name: {}", sink_name))
            | Exec::cmd("grep").arg("-m 1").arg("Volume")
            | Exec::cmd("cut").arg("--delimiter=/").arg("--fields=2")
    }
    .capture()?
    .stdout_str();

    let volume = pactl_sink_volume.split_whitespace().collect::<String>();
    let nommo_volume = NommoVol::try_from(volume).unwrap_or(NommoVol::Muted);
    Ok(nommo_volume)
}

fn set_sink_mute(sink_name: &str, mute: bool) -> subprocess::Result<()> {
    let mute_val = if mute { 1 } else { 0 };
    Exec::shell(format!("pactl set-sink-mute {} {}", sink_name, mute_val))
        .join()
        .map(|_| ())?;

    if mute {
        println!("[nommo] Set mute to true");
    }

    Ok(())
}

fn set_sink_volume(sink_name: &str, vol: NommoVol) -> subprocess::Result<()> {
    set_sink_mute(sink_name, vol == NommoVol::Muted)?;

    Exec::shell(format!("pactl set-sink-volume {} {}", sink_name, vol))
        .join()
        .map(|_| ())?;

    println!("[nommo] Set volume to: {}", vol);

    Ok(())
}

fn handle_device(dev_handle: hidapi::HidDevice, sink_name: &str) {
    let mut buff = [0 as u8; 16];
    loop {
        dev_handle.read(&mut buff).expect("Cannot read from device");
        let msg = NommoMsg::try_from(&buff).expect("Cannot convert data");
        let current_vol = get_sink_volume(&sink_name).expect("Cannot get volume");

        match msg {
            NommoMsg::VolUp => {
                set_sink_volume(&sink_name, current_vol.inc(VOL_STEP))
                    .expect("Cannot increase volume");
            }
            NommoMsg::VolDown => {
                set_sink_volume(&sink_name, current_vol.dec(VOL_STEP))
                    .expect("Cannot decrease volume");
            }
            _ => {}
        }
    }
}

fn main() {
    let default_sink_name = get_default_sink_name().expect("Cannot get default sink name");

    match hidapi::HidApi::new() {
        Ok(api) => {
            let device = api.open(NOMMO_VENDOR_ID, NOMMO_PRODUCT_ID);
            match device {
                Ok(handle) => handle_device(handle, &default_sink_name),
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
