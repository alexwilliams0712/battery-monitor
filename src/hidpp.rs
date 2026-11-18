use crate::battery::{Battery, State};
use async_hid::{AsyncHidRead as _, AsyncHidWrite as _, DeviceReaderWriter, HidBackend, HidResult};
use core::{ops::RangeInclusive, time::Duration};
use futures_lite::StreamExt as _;

const LOGITECH: u16 = 0x046d;
/// Unifying, Lightspeed and Bolt receivers, as listed by Solaar.
const RECEIVERS: [u16; 11] = [
    0xc52b, 0xc532, 0xc539, 0xc53a, 0xc53d, 0xc53f, 0xc541, 0xc545, 0xc547, 0xc54d, 0xc548,
];
const VENDOR_USAGE_PAGE: u16 = 0xff00;
const PAIRED: RangeInclusive<u8> = 1..=6;
/// Asleep or unpaired devices make the receiver answer with an error at once,
/// so this only bounds a lost reply.
const TIMEOUT: Duration = Duration::from_millis(500);

const SHORT: u8 = 0x10;
const LONG: u8 = 0x11;
const ERROR_V1: u8 = 0x8f;
const ERROR_V2: u8 = 0xff;
/// Tags our replies. Solaar lists the IDs other HID++ clients claim;
/// the Linux kernel uses 0x01.
const SOFTWARE_ID: u8 = 0x0c;

const RECEIVER: u8 = 0xff;
const READ_LONG_REGISTER: u8 = 0x83;
const RECEIVER_INFO: u8 = 0xb5;
const CODENAME: u8 = 0x40;

const ROOT: u8 = 0x00;
const DEVICE_NAME: u16 = 0x0005;
const BATTERY_STATUS: u16 = 0x1000;
const BATTERY_VOLTAGE: u16 = 0x1001;
const UNIFIED_BATTERY: u16 = 0x1004;

/// Reply payload after the four header bytes, zero-padded.
type Params = [u8; 16];

type Parse = fn(Params) -> (f64, State);

/// Battery features in order of preference: id, function, parser.
const BATTERY_FEATURES: [(u16, u8, Parse); 3] = [
    (UNIFIED_BATTERY, 1, unified),
    (BATTERY_STATUS, 0, battery_status),
    (BATTERY_VOLTAGE, 0, voltage),
];

/// One HID++ short report.
/// For HID++ 2.0, `sub_id` is the feature index and `address` the function and software ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Request {
    device: u8,
    sub_id: u8,
    address: u8,
    params: [u8; 3],
}

impl Request {
    const fn feature(device: u8, index: u8, function: u8, params: [u8; 3]) -> Self {
        Self {
            device,
            sub_id: index,
            address: (function << 4) | SOFTWARE_ID,
            params,
        }
    }

    const fn codename(device: u8) -> Self {
        Self {
            device: RECEIVER,
            sub_id: READ_LONG_REGISTER,
            address: RECEIVER_INFO,
            params: [CODENAME + device - 1, 0, 0],
        }
    }

    const fn encode(self) -> [u8; 7] {
        let Self {
            device,
            sub_id,
            address,
            params: [a, b, c],
        } = self;
        [SHORT, device, sub_id, address, a, b, c]
    }

    /// `None` if `report` does not answer this request, else its params or HID++ error code.
    fn decode(self, report: &[u8]) -> Option<Result<Params, u8>> {
        let Self {
            device,
            sub_id,
            address,
            params: [first, ..],
        } = self;
        let (&[id, from, a, b], rest) = report.split_first_chunk()?;
        if !matches!((id, report.len()), (SHORT, 7) | (LONG, 20)) || from != device {
            return None;
        }
        if matches!(a, ERROR_V1 | ERROR_V2)
            && b == sub_id
            && let &[echo, code, ..] = rest
            && echo == address
        {
            return Some(Err(code));
        }
        // Register replies echo the sub-register, which tells them apart.
        let echoes = sub_id != READ_LONG_REGISTER || rest.first() == Some(&first);
        if a != sub_id || b != address || !echoes {
            return None;
        }
        let mut params = Params::default();
        params
            .iter_mut()
            .zip(rest)
            .for_each(|(it, byte)| *it = *byte);
        Some(Ok(params))
    }
}

#[test]
fn request_round_trip() {
    let request = Request::feature(2, 0x05, 1, [0, 0, 0]);
    assert_eq!(request.encode(), [0x10, 2, 0x05, 0x1c, 0, 0, 0]);
    let mut reply = [0; 20];
    reply[..7].copy_from_slice(&[0x11, 2, 0x05, 0x1c, 37, 4, 0]);
    let mut params = Params::default();
    params[..2].copy_from_slice(&[37, 4]);
    assert_eq!(request.decode(&reply), Some(Ok(params)));
    // Asleep device, reported by the receiver.
    let asleep = [0x10, 2, 0x8f, 0x05, 0x1c, 0x09, 0];
    assert_eq!(request.decode(&asleep), Some(Err(0x09)));
    // Feature error from the device.
    let mut error = [0; 20];
    error[..6].copy_from_slice(&[0x11, 2, 0xff, 0x05, 0x1c, 0x02]);
    assert_eq!(request.decode(&error), Some(Err(0x02)));
    // Other device, other software ID, DJ report, wrong length.
    for report in [
        &[
            0x11, 3, 0x05, 0x1c, 37, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ][..],
        &[0x10, 2, 0x05, 0x1d, 37, 4, 0],
        &[0x20, 2, 0x05, 0x1c, 37, 4, 0],
        &[0x10, 2, 0x05, 0x1c, 37],
    ] {
        assert_eq!(request.decode(report), None);
    }
}

#[test]
fn codename_reply_must_echo_device() {
    let request = Request::codename(2);
    assert_eq!(request.encode(), [0x10, 0xff, 0x83, 0xb5, 0x41, 0, 0]);
    let mut reply = [0; 20];
    reply[..10].copy_from_slice(&[0x11, 0xff, 0x83, 0xb5, 0x41, 4, b'G', b'5', b'0', b'2']);
    assert!(matches!(request.decode(&reply), Some(Ok(_))));
    reply[4] = 0x40;
    assert_eq!(request.decode(&reply), None);
}

/// Charging state byte shared by features 0x1000 and 0x1004, read as Solaar does.
fn charging(status: u8) -> State {
    match status {
        0 => State::Discharging,
        1 | 2 | 4 => State::Charging,
        3 => State::Full,
        _ => State::Unknown,
    }
}

/// Feature 0x1004 `getStatus`.
fn unified(params: Params) -> (f64, State) {
    let [charge, level, status, ..] = params;
    // Without a state of charge, the level flags give Solaar's approximation.
    let percentage = match (charge, level) {
        (0, 8) => 90,
        (0, 4) => 50,
        (0, 2) => 20,
        (0, 1) => 5,
        (0, _) => 0,
        (it, _) => it,
    };
    (f64::from(percentage), charging(status))
}

/// Feature 0x1000 `getBatteryLevelStatus`.
fn battery_status(params: Params) -> (f64, State) {
    let [level, _next, status, ..] = params;
    (f64::from(level), charging(status))
}

/// Feature 0x1001 `getBatteryVoltage`.
fn voltage(params: Params) -> (f64, State) {
    let [high, low, flags, ..] = params;
    let state = match (flags & 0x80 != 0, flags & 0x03) {
        (false, _) => State::Discharging,
        (true, 1) => State::Full,
        (true, _) => State::Charging,
    };
    let millivolts = u16::from_be_bytes([high, low]);
    (f64::from(voltage_percentage(millivolts)), state)
}

#[test]
fn battery_replies() {
    let params = |bytes: &[u8]| {
        let mut it = Params::default();
        it[..bytes.len()].copy_from_slice(bytes);
        it
    };
    assert_eq!(unified(params(&[37, 4, 0])), (37.0, State::Discharging));
    assert_eq!(unified(params(&[0, 2, 1])), (20.0, State::Charging));
    assert_eq!(battery_status(params(&[50, 20, 3])), (50.0, State::Full));
    // 3811 mV, discharging.
    assert_eq!(
        voltage(params(&[0x0e, 0xe3, 0x00])),
        (50.0, State::Discharging)
    );
    assert_eq!(
        voltage(params(&[0x0e, 0xe3, 0x80])),
        (50.0, State::Charging)
    );
    assert_eq!(voltage(params(&[0x0e, 0xe3, 0x81])), (50.0, State::Full));
}

/// Li-ion discharge curve from Solaar, in millivolts and percent.
const CURVE: [(u16, u8); 13] = [
    (4186, 100),
    (4067, 90),
    (3989, 80),
    (3922, 70),
    (3859, 60),
    (3811, 50),
    (3778, 40),
    (3751, 30),
    (3717, 20),
    (3671, 10),
    (3646, 5),
    (3579, 2),
    (3500, 0),
];

/// Linear interpolation on [`CURVE`], clamped to 0..=100.
fn voltage_percentage(millivolts: u16) -> u8 {
    CURVE
        .array_windows()
        .find(|[_, (low, _)]| millivolts >= *low)
        .map_or(0, |&[(v_high, p_high), (v_low, p_low)]| {
            let t = (f64::from(millivolts - v_low) / f64::from(v_high - v_low)).min(1.0);
            let percent = f64::from(p_low) + f64::from(p_high - p_low) * t;
            // In 0..=100 by construction.
            percent.round() as u8
        })
}

#[test]
fn voltage_curve() {
    for (millivolts, percent) in [
        (5000, 100),
        (4186, 100),
        (4067, 90),
        (3811, 50),
        (3800, 47),
        (3500, 0),
        (3000, 0),
    ] {
        assert_eq!(voltage_percentage(millivolts), percent, "{millivolts} mV");
    }
}

/// Model in the form the Linux kernel gives UPower, e.g. `Logitech G502`.
fn model(name: &[u8]) -> Option<String> {
    let name = String::from_utf8_lossy(name);
    let name = name.trim_matches(|it: char| it == '\0' || it.is_whitespace());
    match (name.is_empty(), name.starts_with("Logitech ")) {
        (true, _) => None,
        (false, true) => Some(name.into()),
        (false, false) => Some(format!("Logitech {name}")),
    }
}

#[test]
fn models() {
    assert_eq!(model(b"G502\0\0"), Some("Logitech G502".into()));
    assert_eq!(model(b"Logitech MX Keys"), Some("Logitech MX Keys".into()));
    assert_eq!(model(b"\0\0"), None);
}

/// Batteries of devices paired with Logitech receivers, read over HID++ 2.0.
pub async fn batteries() -> HidResult<Vec<Battery>> {
    let backend = HidBackend::default();
    let mut receivers = backend.enumerate().await?.filter(|it| {
        it.vendor_id == LOGITECH
            && it.usage_page == VENDOR_USAGE_PAGE
            && RECEIVERS.contains(&it.product_id)
    });
    // Each HID++ collection of a receiver is listed separately.
    let mut seen = Vec::new();
    let mut out = Vec::new();
    while let Some(receiver) = receivers.next().await {
        if seen.contains(&receiver.id) {
            continue;
        }
        seen.push(receiver.id.clone());
        let mut io = receiver.open().await?;
        for device in PAIRED {
            out.extend(battery(&mut io, device).await?);
        }
    }
    out.sort_by(|a, b| a.model.cmp(&b.model));
    Ok(out)
}

async fn battery(io: &mut DeviceReaderWriter, device: u8) -> HidResult<Option<Battery>> {
    let ping = Request::feature(device, ROOT, 1, [0, 0, 0x5a]);
    if call(io, ping).await?.is_none() {
        return Ok(None);
    }
    for (id, function, parse) in BATTERY_FEATURES {
        if let Some(index) = feature(io, device, id).await?
            && let Some(params) =
                call(io, Request::feature(device, index, function, [0; 3])).await?
        {
            let (percentage, state) = parse(params);
            return Ok(Some(Battery {
                model: name(io, device).await?,
                percentage,
                state,
                icon_name: String::new(),
            }));
        }
    }
    Ok(None)
}

/// Index of feature `id`, if the device has it.
async fn feature(io: &mut DeviceReaderWriter, device: u8, id: u16) -> HidResult<Option<u8>> {
    let [high, low] = id.to_be_bytes();
    let reply = call(io, Request::feature(device, ROOT, 0, [high, low, 0])).await?;
    Ok(reply.and_then(|[index, ..]| (index != 0).then_some(index)))
}

/// The receiver's codename, as Linux uses, else the device's marketing name.
async fn name(io: &mut DeviceReaderWriter, device: u8) -> HidResult<String> {
    if let Some([_, len, codename @ ..]) = call(io, Request::codename(device)).await?
        && let Some(it) = model(
            codename
                .get(..usize::from(len))
                .unwrap_or(codename.as_slice()),
        )
    {
        return Ok(it);
    }
    if let Some(index) = feature(io, device, DEVICE_NAME).await?
        && let Some([len, ..]) = call(io, Request::feature(device, index, 0, [0; 3])).await?
    {
        let len = usize::from(len);
        let mut bytes = Vec::with_capacity(len);
        while bytes.len() < len {
            let offset = u8::try_from(bytes.len()).expect("offset MUST be below a u8 length");
            match call(io, Request::feature(device, index, 1, [offset, 0, 0])).await? {
                Some(chunk) => bytes.extend(chunk.iter().take(len - bytes.len())),
                None => break,
            }
        }
        if let Some(it) = model(&bytes) {
            return Ok(it);
        }
    }
    Ok(format!("Logitech device {device}"))
}

/// Sends `request` and waits for its reply; `None` on a HID++ error or timeout.
async fn call(io: &mut DeviceReaderWriter, request: Request) -> HidResult<Option<Params>> {
    io.write_output_report(&request.encode()).await?;
    let reply = async {
        let mut buf = [0; 64];
        loop {
            let len = io.read_input_report(&mut buf).await?;
            if let Some(reply) = request.decode(buf.get(..len).unwrap_or_default()) {
                return HidResult::Ok(reply.ok());
            }
        }
    };
    tokio::time::timeout(TIMEOUT, reply)
        .await
        .unwrap_or(Ok(None))
}
