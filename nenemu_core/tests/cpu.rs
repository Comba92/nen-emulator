#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct CpuTest<'a> {
    name: &'a str,
    #[serde(rename = "initial")]
    start: CpuTestState,
    #[serde(rename = "final")]
    end: CpuTestState,
    cycles: Vec<(usize, usize, &'a str)>,
}

#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct CpuTestState {
    pc: u16,
    s: u8,
    a: u8,
    x: u8,
    y: u8,
    p: u8,
    ram: Vec<(usize, usize)>,
}

use std::{
    fs,
    io::{self, Read},
};

use nenemu_core::{cpu::Status, emu};

fn cpu_to_mock(emu: &mut emu::NesEmulator, mock: &CpuTestState) -> CpuTestState {
    let cpu = &emu.cpu;

    let mut test = CpuTestState {
        a: cpu.a,
        x: cpu.x,
        y: cpu.y,
        pc: cpu.pc,
        s: cpu.sp,
        p: cpu.p.bits(),
        ram: Vec::new(),
    };

    for (addr, _) in &mock.ram {
        test.ram
            .push((*addr, emu.cpu_read16(*addr as u16) as usize));
    }

    test
}

fn cpu_from_mock(emu: &mut emu::NesEmulator, mock: &CpuTestState) {
    let cpu = &mut emu.cpu;

    cpu.a = mock.a;
    cpu.x = mock.x;
    cpu.y = mock.y;
    cpu.pc = mock.pc;
    cpu.sp = mock.s;
    cpu.p = Status::from_bits_retain(mock.p);

    for (addr, val) in &mock.ram {
        emu.cpu_dispatch_write(*addr as u16, *val as u8);
    }
}

#[test]
fn parse_test() {
    let res: Vec<CpuTest> =
        serde_json::from_str(include_str!("./SingleStepTests/00.json")).unwrap();

    println!("{:?}", res[0]);
}

#[test]
fn exec_test() {
    let test: Vec<CpuTest> =
        serde_json::from_str(include_str!("./SingleStepTests/a9.json")).unwrap();

    let mut emu = emu::NesEmulator::debug();
    println!("{:?}", emu.cpu);

    cpu_from_mock(&mut emu, &test[0].start);

    println!("{:?}", test[0]);

    while emu.cpu.cycles < test[0].cycles.len() {
        emu.cpu_step();
    }

    let res = cpu_to_mock(&mut emu, &test[0].end);

    assert_eq!(res, test[0].end);
}

use pretty_assertions::assert_eq;

fn cpu_test(emu: &mut emu::NesEmulator, test: &CpuTest) -> bool {
    cpu_from_mock(emu, &test.start);

    emu.cpu_step();

    let res = cpu_to_mock(emu, &test.end);

    // clear written addresses
    for (addr, _) in &res.ram {
        emu.cpu_dispatch_write(*addr as u16, 0);
    }

    assert_eq!(res, test.end, "{}", test.name);
    res == test.end
}

const LEGALS: &[&str] = &[
    "00", "01", "05", "06", "08", "09", "0a", "0d", "0e", "10", "11", "15", "16", "18", "19", "1d",
    "1e", "20", "21", "24", "25", "26", "28", "29", "2a", "2c", "2d", "2e", "30", "31", "35", "36",
    "38", "39", "3d", "3e", "40", "41", "45", "46", "48", "49", "4a", "4c", "4d", "4e", "50", "51",
    "55", "56", "58", "59", "5d", "5e", "60", "61", "65", "66", "68", "69", "6a", "6c", "6d", "6e",
    "70", "71", "75", "76", "78", "79", "7d", "7e", "81", "84", "85", "86", "88", "8a", "8c", "8d",
    "8e", "90", "91", "94", "95", "96", "98", "99", "9a", "9d", "a0", "a1", "a2", "a4", "a5", "a6",
    "a8", "a9", "aa", "ac", "ad", "ae", "b0", "b1", "b4", "b5", "b6", "b8", "b9", "ba", "bc", "bd",
    "be", "c0", "c1", "c4", "c5", "c6", "c8", "c9", "ca", "cc", "cd", "ce", "d0", "d1", "d5", "d6",
    "d8", "d9", "dd", "de", "e0", "e1", "e4", "e5", "e6", "e8", "e9", "ea", "ec", "ed", "ee", "f0",
    "f1", "f5", "f6", "f8", "f9", "fd", "fe",
];

#[test]
fn exec_all_tests() {
    let files = fs::read_dir("./tests/SingleStepTests").expect("tests folder missing");
    let mut file_str = String::new();

    let mut emu = emu::NesEmulator::debug();

    for file in files {
        let entry = file.unwrap();

        if !LEGALS.contains(&&entry.file_name().to_str().unwrap()[0..2]) {
            continue;
        }

        let file = fs::File::open(entry.path()).unwrap();
        let mut file_buf = io::BufReader::new(file);
        file_buf.read_to_string(&mut file_str).unwrap();

        let tests: Vec<CpuTest> = serde_json::from_str(&file_str).unwrap();
        for test in tests {
            cpu_test(&mut emu, &test);
        }

        file_str.clear();
    }
}
