use std::convert::TryFrom;
use subprocess::Exec;

use libpulse_binding::volume::{ChannelVolumes, Volume, VOLUME_NORM};
use pulsectl::controllers::DeviceControl;
use pulsectl::controllers::SinkController;

const NOMMO_VENDOR_ID: u16 = 0x1532;
const NOMMO_PRODUCT_ID: u16 = 0x0517;
const VOL_DELTA: f64 = 0.05;

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
            _ => Ok(Self::Noop),
        }
    }
}

fn volume_from_percent(delta: f64) -> Volume {
    let vol_raw = (delta * 100.0) * (f64::from(VOLUME_NORM.0) / 100.0);
    Volume::from(Volume(vol_raw as u32))
}

fn set_volume(volumes: &ChannelVolumes, sink_controller: &mut SinkController, sink_index: u32) {
    let op = sink_controller
        .handler
        .introspect
        .set_sink_volume_by_index(sink_index, &volumes, None);
    sink_controller
        .handler
        .wait_for_operation(op)
        .expect("Error setting volume");
}

fn set_mute(mute: bool, sink_controller: &mut SinkController, sink_index: u32) {
    let op = sink_controller
        .handler
        .introspect
        .set_sink_mute_by_index(sink_index, mute, None);

    sink_controller
        .handler
        .wait_for_operation(op)
        .expect("Error setting volume");
}

fn handle_device(dev_handle: hidapi::HidDevice) {
    // get Pulse Audio default device
    let mut sink_controller = SinkController::create();

    let mut buff = [0 as u8; 16];
    loop {
        let default_sink = sink_controller
            .get_default_device()
            .expect("Cannot get PulseAudio default sink");
        let mut current_volume = default_sink.clone().volume;

        dev_handle.read(&mut buff).expect("Cannot read from device");
        let msg = NommoMsg::try_from(&buff).expect("Cannot convert data");
        match msg {
            NommoMsg::VolUp => {
                let volumes = current_volume
                    .inc_clamp(volume_from_percent(VOL_DELTA), volume_from_percent(1.0))
                    .expect("Cannot set new ChannelVolumes");
                set_volume(volumes, &mut sink_controller, default_sink.index);

                // if muted, unmute
                if default_sink.mute {
                    set_mute(false, &mut sink_controller, default_sink.index);
                }
            }
            NommoMsg::VolDown => {
                let volumes = current_volume
                    .decrease(volume_from_percent(VOL_DELTA))
                    .expect("Cannot set new ChannelVolumes");
                set_volume(volumes, &mut sink_controller, default_sink.index);

                // if volume at 0%, mute
                if default_sink.volume == volume_from_percent(0.0) && !default_sink.mute {
                    set_mute(true, &mut sink_controller, default_sink.index);
                }
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
