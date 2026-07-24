use hidapi::{HidApi, HidDevice};
use std::error::Error;
use std::io;
use std::thread::sleep;
use std::time::Duration;

const VENDOR_ID: u16 = 0x3842;
const PRODUCT_ID: u16 = 0x2622;
const INTERFACE: i32 = 1;
const REPORT_SIZE: usize = 17;
const KEYS_YAML: &str = include_str!("../keys.yaml");

#[derive(Clone, Copy)]
struct EKey {
    name: &'static str,
    position: u8,
}

const E_KEYS: [EKey; 5] = [
    EKey {
        name: "E1",
        position: 0x15,
    },
    EKey {
        name: "E2",
        position: 0x2b,
    },
    EKey {
        name: "E3",
        position: 0x41,
    },
    EKey {
        name: "E4",
        position: 0x52,
    },
    EKey {
        name: "E5",
        position: 0x66,
    },
];

#[derive(Clone, Copy)]
struct Mapping {
    function: u8,
    modifier: u8,
    key1: u8,
    key2: u8,
}

impl Mapping {
    fn keyboard(usage: u8) -> Self {
        let (modifier, key1) = if (0xe0..=0xe7).contains(&usage) {
            (1 << (usage - 0xe0), 0x00)
        } else {
            (0x00, usage)
        };

        Self {
            function: 0x00,
            modifier,
            key1,
            key2: 0x00,
        }
    }

    fn keyboard_usage(self) -> Option<u8> {
        if self.function != 0x00 || self.key2 != 0x00 {
            return None;
        }
        if self.modifier == 0x00 {
            return Some(self.key1);
        }
        if self.key1 == 0x00 && self.modifier.is_power_of_two() {
            return Some(0xe0 + self.modifier.trailing_zeros() as u8);
        }
        None
    }

    fn disabled() -> Self {
        Self {
            function: 0xff,
            modifier: 0x00,
            key1: 0x00,
            key2: 0x00,
        }
    }
}

struct Keyboard {
    device: HidDevice,
}

impl Keyboard {
    fn open() -> Result<Self, Box<dyn Error>> {
        let api = HidApi::new()?;
        let info = api
            .device_list()
            .find(|device| {
                device.vendor_id() == VENDOR_ID
                    && device.product_id() == PRODUCT_ID
                    && device.interface_number() == INTERFACE
            })
            .ok_or_else(|| error("EVGA Z12 (3842:2622, interface 1) not found"))?;

        Ok(Self {
            device: info.open_device(&api)?,
        })
    }

    fn transact(
        &self,
        request: &mut [u8; REPORT_SIZE],
        wait_ms: u64,
    ) -> Result<(), Box<dyn Error>> {
        self.device.send_feature_report(request)?;

        sleep(Duration::from_millis(wait_ms));
        request.fill(0);
        request[0] = 0x04;

        let read = self.device.get_feature_report(request)?;
        if read != REPORT_SIZE {
            return Err(error(format!("read only {read} of {REPORT_SIZE} bytes")));
        }
        if request[6] != 0xc0 {
            return Err(error(format!(
                "keyboard returned error 0x{:02X}",
                request[6]
            )));
        }
        Ok(())
    }

    fn read_key(&self, key: EKey) -> Result<Mapping, Box<dyn Error>> {
        let mut report = [0u8; REPORT_SIZE];
        report[..8].copy_from_slice(&[0x04, 0xea, 0x02, 0x07, 0x01, 0x00, 0x00, key.position]);
        self.transact(&mut report, 10)?;

        Ok(Mapping {
            function: report[8],
            modifier: report[9],
            key1: report[10],
            key2: report[11],
        })
    }

    fn write_key(&self, key: EKey, mapping: Mapping) -> Result<(), Box<dyn Error>> {
        let mut report = [0u8; REPORT_SIZE];
        report[..12].copy_from_slice(&[
            0x04,
            0xea,
            0x02,
            0x07,
            0x00,
            0x00,
            0x00,
            key.position,
            mapping.function,
            mapping.modifier,
            mapping.key1,
            mapping.key2,
        ]);
        self.transact(&mut report, 10)
    }

    fn save(&self) -> Result<(), Box<dyn Error>> {
        let mut report = [0u8; REPORT_SIZE];
        report[..8].copy_from_slice(&[0x04, 0xea, 0x02, 0x12, 0x00, 0x00, 0x00, 0x00]);
        self.transact(&mut report, 300)
    }
}

fn error(message: impl Into<String>) -> Box<dyn Error> {
    io::Error::other(message.into()).into()
}

fn parse_yaml_line(line: &str) -> Option<(&str, u8)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    let (name, value) = line.split_once(':')?;
    let value = value.trim().strip_prefix("0x")?;
    Some((name.trim(), u8::from_str_radix(value, 16).ok()?))
}

fn key_usage(name: &str) -> Option<u8> {
    KEYS_YAML
        .lines()
        .filter_map(parse_yaml_line)
        .find_map(|(known_name, usage)| known_name.eq_ignore_ascii_case(name).then_some(usage))
}

fn key_name(usage: u8) -> Option<&'static str> {
    KEYS_YAML
        .lines()
        .filter_map(parse_yaml_line)
        .find_map(|(name, known_usage)| (known_usage == usage).then_some(name))
}

fn e_key(name: &str) -> Option<EKey> {
    E_KEYS
        .iter()
        .copied()
        .find(|key| key.name.eq_ignore_ascii_case(name))
}

fn print_mapping(key: EKey, mapping: Mapping) {
    print!("{}: ", key.name);
    if mapping.function == 0xff {
        println!("disabled");
    } else if let Some(usage) = mapping.keyboard_usage() {
        match key_name(usage) {
            Some(name) => println!("{name}"),
            None => println!("HID 0x{usage:02X}"),
        }
    } else {
        println!(
            "function=0x{:02X} parameters={:02X},{:02X},{:02X}",
            mapping.function, mapping.modifier, mapping.key1, mapping.key2
        );
    }
}

fn status(keyboard: &Keyboard) -> Result<(), Box<dyn Error>> {
    for key in E_KEYS {
        print_mapping(key, keyboard.read_key(key)?);
    }
    Ok(())
}

fn auto() -> Result<(), Box<dyn Error>> {
    let keyboard = Keyboard::open()?;
    for (index, key) in E_KEYS.into_iter().enumerate() {
        keyboard.write_key(key, Mapping::keyboard(0x68 + index as u8))?;
    }
    keyboard.save()?;
    status(&keyboard)
}

fn set(key_name_arg: &str, target_name: &str) -> Result<(), Box<dyn Error>> {
    let key = e_key(key_name_arg).ok_or_else(|| {
        error(format!(
            "unknown E-key: {key_name_arg}; expected E1 through E5"
        ))
    })?;
    let usage = key_usage(target_name).ok_or_else(|| {
        error(format!(
            "unknown key: {target_name}; nothing was written. See keys.yaml for valid names"
        ))
    })?;

    let keyboard = Keyboard::open()?;
    keyboard.write_key(key, Mapping::keyboard(usage))?;
    keyboard.save()?;
    print_mapping(key, keyboard.read_key(key)?);
    Ok(())
}

fn disable(target: &str) -> Result<(), Box<dyn Error>> {
    let keys: &[EKey] = if target.eq_ignore_ascii_case("all") {
        &E_KEYS
    } else {
        std::slice::from_ref(
            E_KEYS
                .iter()
                .find(|key| key.name.eq_ignore_ascii_case(target))
                .ok_or_else(|| {
                    error(format!(
                        "unknown E-key: {target}; expected E1 through E5 or all"
                    ))
                })?,
        )
    };

    let keyboard = Keyboard::open()?;
    for &key in keys {
        keyboard.write_key(key, Mapping::disabled())?;
    }
    keyboard.save()?;
    for &key in keys {
        print_mapping(key, keyboard.read_key(key)?);
    }
    Ok(())
}

fn usage(program: &str) {
    eprintln!("Usage:");
    eprintln!("  {program} status");
    eprintln!("  {program} auto");
    eprintln!("  {program} set E1 KEY");
    eprintln!("  {program} disable E1|all");
}

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    match args.as_slice() {
        [_, command] if command == "status" => status(&Keyboard::open()?),
        [_, command] if command == "auto" => auto(),
        [_, command, key, target] if command == "set" => set(key, target),
        [_, command, target] if command == "disable" => disable(target),
        [program, ..] => {
            usage(program);
            Err(error("invalid command"))
        }
        [] => unreachable!(),
    }
}

fn main() {
    if let Err(problem) = run() {
        eprintln!("Error: {problem}");
        std::process::exit(1);
    }
}
