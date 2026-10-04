//! Golden bytes for the protocol pieces, and the client against a fake SDK server on 127.0.0.1:0 (never a
//! real OpenRGB).

use super::*;
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;

/// Test-only encoder for a controller data block, mirroring OpenRGBSDK.md for `version`.
struct Block(Vec<u8>);

impl Block {
    fn u16(&mut self, v: u16) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    fn u32(&mut self, v: u32) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    fn text(&mut self, s: &str) -> &mut Self {
        self.u16(s.len() as u16 + 1);
        self.0.extend_from_slice(s.as_bytes());
        self.0.push(0);
        self
    }
}

/// `(name, type, leds, matrix (h, w, map))` per zone.
type TestZone<'a> = (&'a str, i32, u32, Option<(u32, u32, Vec<u32>)>);

fn controller_block(version: u32, kind: i32, name: &str, vendor: &str, zones: &[TestZone], flags: u32) -> Vec<u8> {
    let mut b = Block(vec![]);
    b.u32(0).u32(kind as u32).text(name);
    if version >= 1 {
        b.text(vendor);
    }
    b.text("description").text("1.0").text("SERIAL-NOT-KEPT").text("I2C: bus 0");
    // two modes, the second with two colours
    b.u16(2).u32(1);
    for (i, mode) in ["Direct", "Rainbow"].iter().enumerate() {
        b.text(mode).u32(i as u32).u32(0x20).u32(0).u32(0);
        if version >= 3 {
            b.u32(0).u32(100);
        }
        b.u32(0).u32(0).u32(0);
        if version >= 3 {
            b.u32(100);
        }
        b.u32(0).u32(1).u16(i as u16 * 2);
        for _ in 0..i * 2 {
            b.u32(0x00FF_0000);
        }
    }
    b.u16(zones.len() as u16);
    for (zname, ztype, leds, matrix) in zones {
        b.text(zname).u32(*ztype as u32).u32(*leds).u32(*leds).u32(*leds);
        match matrix {
            Some((h, w, map)) => {
                b.u16(8 + 4 * map.len() as u16).u32(*h).u32(*w);
                for m in map {
                    b.u32(*m);
                }
            }
            None => {
                b.u16(0);
            }
        }
        if version >= 4 {
            b.u16(1).text("segment").u32(1).u32(0).u32(*leds);
        }
        if version >= 5 {
            b.u32(0);
        }
    }
    let total: u32 = zones.iter().map(|z| z.2).sum();
    b.u16(total as u16);
    for i in 0..total {
        b.text(&format!("LED {i}")).u32(i);
    }
    b.u16(total as u16);
    for _ in 0..total {
        b.u32(0);
    }
    if version >= 5 {
        b.u16(1).text("alt");
        b.u32(flags);
    }
    let size = b.0.len() as u32;
    b.0[..4].copy_from_slice(&size.to_le_bytes());
    b.0
}

fn board_zones() -> Vec<TestZone<'static>> {
    vec![
        ("Aura Mainboard", 1, 3, None),
        ("Logo", 0, 1, None),
        ("Panel", 2, 4, Some((2, 3, vec![0, 1, u32::MAX, 2, 3, u32::MAX]))),
    ]
}

#[test]
fn header_and_update_leds_golden_bytes() {
    let p = packet(2, id::UPDATE_LEDS, &update_leds_payload(&[[1, 2, 3], [255, 0, 128]]));
    assert_eq!(
        p,
        [
            b'O', b'R', b'G', b'B', 2, 0, 0, 0, 0x1A, 0x04, 0, 0, 14, 0, 0, 0, // header: dev 2, id 1050, size 14
            14, 0, 0, 0, 2, 0, 1, 2, 3, 0, 255, 0, 128, 0, // data_size, count, R G B 0 each
        ]
    );
    let h: [u8; 16] = p[..16].try_into().unwrap();
    assert_eq!(header(&h).unwrap(), (2, 1050, 14));
    let mut wrong = h;
    wrong[0] = b'X';
    assert!(header(&wrong).is_err());
    let mut huge = h;
    huge[12..16].copy_from_slice(&(MAX_PACKET + 1).to_le_bytes());
    assert!(header(&huge).is_err(), "oversized packets are refused before allocating");
}

#[test]
fn controller_data_parses_for_every_version() {
    for version in 0..=MAX_VERSION {
        let block = controller_block(version, device_type::MOTHERBOARD, "ASUS ROG STRIX", "ASUS", &board_zones(), 0);
        let c = parse_controller(&block, version).unwrap_or_else(|e| panic!("v{version}: {e}"));
        assert_eq!(c.kind, 0);
        assert_eq!(c.name, "ASUS ROG STRIX");
        assert_eq!(c.vendor, if version >= 1 { "ASUS" } else { "" });
        assert_eq!(c.zones.len(), 3);
        assert_eq!(c.zones[2].matrix.as_ref().unwrap().map, [0, 1, u32::MAX, 2, 3, u32::MAX]);
        assert_eq!((c.leds.len(), c.colors), (8, 8));
        assert_eq!(c.leds[7], "LED 7");
        assert!(!format!("{c:?}").contains("SERIAL"), "the serial is never kept");
        // cut anywhere: a clean error, never a panic
        for cut in [5, block.len() / 2, block.len() - 1] {
            assert!(parse_controller(&block[..cut], version).is_err(), "v{version} cut at {cut}");
        }
    }
    let hidden = controller_block(5, 2, "GPU", "NVIDIA", &[("GPU", 1, 2, None)], CONTROLLER_FLAG_HIDDEN);
    assert_eq!(parse_controller(&hidden, 5).unwrap().flags, CONTROLLER_FLAG_HIDDEN);
}

#[test]
fn names_lose_control_characters() {
    let block = controller_block(5, 2, "Ge\u{1b}[31mForce\n", "NVIDIA", &[("GPU", 1, 1, None)], 0);
    assert_eq!(parse_controller(&block, 5).unwrap().name, "Ge[31mForce");
}

#[test]
fn razer_detector_list_is_there() {
    let names: Vec<&str> = razer_detectors().collect();
    assert!(names.len() > 150, "{}", names.len());
    assert!(names.contains(&"Razer Blackwidow V4 Pro 75%") || names.iter().any(|n| n.contains("Blackwidow V4")));
    assert!(names.iter().all(|n| n.contains("Razer") && !n.contains('"') && !n.contains('\\')));
}

/// What the fake server saw.
#[derive(Debug, PartialEq)]
enum Seen {
    Name(String),
    CustomMode(u32),
    Leds(u32, Vec<u8>),
}

/// A fake SDK server speaking `server_version` with these controller blocks. Sends DEVICE_LIST_UPDATED
/// right after the first UpdateLEDs when `push_update` is set.
fn fake_server(
    server_version: Option<u32>,
    controllers: Vec<Vec<u8>>,
    push_update: bool,
) -> (u16, mpsc::Receiver<Seen>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let mut pushed = false;
        loop {
            let mut h = [0u8; 16];
            if s.read_exact(&mut h).is_err() {
                return;
            }
            let (dev, id, size) = header(&h).unwrap();
            let mut p = vec![0u8; size as usize];
            s.read_exact(&mut p).unwrap();
            let reply = |s: &mut TcpStream, id: u32, payload: &[u8]| s.write_all(&packet(0, id, payload)).unwrap();
            match id {
                id::REQUEST_PROTOCOL_VERSION => {
                    if let Some(v) = server_version {
                        // unrelated packets first, as newer servers send their name
                        reply(&mut s, 51, b"OpenRGB\0");
                        reply(&mut s, id, &v.to_le_bytes());
                    }
                }
                id::SET_CLIENT_NAME => tx.send(Seen::Name(String::from_utf8_lossy(&p).into())).unwrap(),
                id::REQUEST_CONTROLLER_COUNT => reply(&mut s, id, &(controllers.len() as u32).to_le_bytes()),
                id::REQUEST_CONTROLLER_DATA => {
                    let asked = if p.len() == 4 { u32::from_le_bytes([p[0], p[1], p[2], p[3]]) } else { 0 };
                    assert_eq!(asked, server_version.unwrap_or(0).min(MAX_VERSION));
                    reply(&mut s, id, &controllers[dev as usize]);
                }
                id::SET_CUSTOM_MODE => tx.send(Seen::CustomMode(dev)).unwrap(),
                id::UPDATE_LEDS => {
                    tx.send(Seen::Leds(dev, p)).unwrap();
                    if push_update && !pushed {
                        pushed = true;
                        reply(&mut s, id::DEVICE_LIST_UPDATED, &[]);
                    }
                }
                other => panic!("unexpected packet {other}"),
            }
        }
    });
    (port, rx)
}

#[test]
fn client_negotiates_lists_and_drives_a_fake_server() {
    let v = 6; // a 1.0 server: the client stays at 5
    let blocks = vec![
        controller_block(5, 0, "ASUS ROG STRIX", "ASUS", &board_zones(), 0),
        controller_block(5, 2, "GeForce RTX", "NVIDIA", &[("GPU", 1, 2, None)], 0),
    ];
    let (port, seen) = fake_server(Some(v), blocks, true);
    let mut c = Client::connect(port, "uncoil", Duration::from_secs(2)).unwrap();
    assert_eq!(c.version(), 5);
    let list = c.controllers().unwrap();
    assert_eq!(list.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["ASUS ROG STRIX", "GeForce RTX"]);
    c.set_custom_mode(1).unwrap();
    c.update_leds(1, &[[10, 20, 30], [0, 0, 0]]).unwrap();
    assert_eq!(seen.recv().unwrap(), Seen::Name("uncoil\0".into()));
    assert_eq!(seen.recv().unwrap(), Seen::CustomMode(1));
    assert_eq!(seen.recv().unwrap(), Seen::Leds(1, update_leds_payload(&[[10, 20, 30], [0, 0, 0]])));
    // the server's "device list updated" arrives without blocking the sender
    let deadline = Instant::now() + Duration::from_secs(2);
    while !c.poll().unwrap() {
        assert!(Instant::now() < deadline, "the update notice never arrived");
        thread::sleep(Duration::from_millis(5));
    }
    assert!(!{
        c.controllers().unwrap();
        c.poll().unwrap()
    });
}

#[test]
fn version_0_and_4_servers() {
    let (port, _seen) =
        fake_server(None, vec![controller_block(0, 1, "Vengeance", "", &[("DRAM", 1, 10, None)], 0)], false);
    let mut c = Client::connect(port, "uncoil", Duration::from_secs(2)).unwrap();
    assert_eq!(c.version(), 0, "no answer to the version request means version 0");
    assert_eq!(c.controllers().unwrap()[0].leds.len(), 10);

    let (port, _seen) =
        fake_server(Some(4), vec![controller_block(4, 1, "Vengeance", "Corsair", &[("DRAM", 1, 10, None)], 0)], false);
    let mut c = Client::connect(port, "uncoil", Duration::from_secs(2)).unwrap();
    assert_eq!(c.version(), 4);
    assert_eq!(c.controllers().unwrap()[0].vendor, "Corsair");
}

#[test]
fn a_closed_server_is_an_error_not_a_hang() {
    let (port, _seen) = fake_server(Some(5), vec![], false);
    let mut c = Client::connect(port, "uncoil", Duration::from_secs(2)).unwrap();
    assert!(c.controllers().unwrap().is_empty());
    drop(_seen);
    // the fake ends when its receiver is gone and the next send fails; then poll reports the close
    let _ = c.update_leds(0, &[[1, 1, 1]]);
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match c.poll() {
            Err(_) => break,
            Ok(_) => {
                assert!(Instant::now() < deadline, "a closed connection must surface as an error");
                let _ = c.update_leds(0, &[[1, 1, 1]]);
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
    // nothing listening at all: connecting fails fast
    let free = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    assert!(Client::connect(free, "uncoil", Duration::from_millis(500)).is_err());
}
