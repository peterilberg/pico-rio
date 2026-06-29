use embassy_time::Duration;
use messages::{Command, Value};

use crate::bang_bang;
use crate::display;
use crate::sequencer::{Guard, Sequence, Step};

const FILL: u8 = 27;
const PUMP: u8 = 26;
const SOURCE: u8 = 20;
const DRAIN: u8 = 19;
const EMERGENCY: u8 = 21;

const SET_PUMP: u8 = 6;
const SET_SOURCE: u8 = 11;
const SET_DRAIN: u8 = 12;

const CONTROLLER_LOWER_LIMIT: u8 = 45;
const CONTROLLER_UPPER_LIMIT: u8 = 50;

#[embassy_executor::task]
pub async fn task() {
    display::add_text("Water tank", Value::None).await;
    display::add_text("Pump", Value::Analog(PUMP)).await;
    display::add_text("Fill level", Value::Analog(FILL)).await;
    display::add_text("Source (NC)", Value::OffOn(SOURCE)).await;
    display::add_text("Drain  (NO)", Value::OnOff(DRAIN)).await;

    bang_bang::set_input(FILL).await;
    bang_bang::set_output(SET_PUMP).await;
    bang_bang::set_lower_limit(CONTROLLER_LOWER_LIMIT).await;
    bang_bang::set_upper_limit(CONTROLLER_UPPER_LIMIT).await;
}

pub static SEQUENCES: &[Sequence] = &[Sequence {
    id: 1,
    guards: &[
        Guard {
            id: 1,
            condition: |measurements| measurements[FILL as usize] > 80,
        },
        Guard {
            id: 2,
            condition: |measurements| measurements[EMERGENCY as usize] == 0,
        },
    ],
    steps: &[
        // Step 1
        Step::At(Duration::from_secs(0)),
        Step::AddLine {
            label: "Fill sequence #",
            value: Value::Number(1),
        },
        Step::AddLine {
            label: "Controller",
            value: Value::Boolean(false),
        },
        Step::AddLine {
            label: "Pump",
            value: Value::Analog(PUMP),
        },
        Step::AddLine {
            label: "Source (NC)",
            value: Value::OffOn(SOURCE),
        },
        Step::AddLine {
            label: "Drain (NO)",
            value: Value::OnOff(DRAIN),
        },
        Step::Do(Command::BangBangStop),
        Step::Do(Command::BangBangHide),
        Step::Do(Command::SetDO {
            pin: SET_DRAIN,
            value: false,
        }),
        // Step 2
        Step::At(Duration::from_secs(3)),
        Step::AddLine {
            label: "Fill sequence #",
            value: Value::Number(2),
        },
        Step::AddLine {
            label: "Controller",
            value: Value::Boolean(false),
        },
        Step::AddLine {
            label: "Pump",
            value: Value::Analog(PUMP),
        },
        Step::AddLine {
            label: "Source (NC)",
            value: Value::OffOn(SOURCE),
        },
        Step::AddLine {
            label: "Drain (NO)",
            value: Value::OnOff(DRAIN),
        },
        Step::Do(Command::SetAO {
            pin: SET_PUMP,
            value: 0,
        }),
        // Step 3
        Step::At(Duration::from_secs(6)),
        Step::AddLine {
            label: "Fill sequence #",
            value: Value::Number(3),
        },
        Step::AddLine {
            label: "Controller",
            value: Value::Boolean(false),
        },
        Step::AddLine {
            label: "Pump",
            value: Value::Analog(PUMP),
        },
        Step::AddLine {
            label: "Source (NC)",
            value: Value::OffOn(SOURCE),
        },
        Step::AddLine {
            label: "Drain (NO)",
            value: Value::OnOff(DRAIN),
        },
        Step::Do(Command::SetDO {
            pin: SET_SOURCE,
            value: true,
        }),
        Step::Do(Command::SetDO {
            pin: SET_DRAIN,
            value: true,
        }),
        // Step 4
        Step::At(Duration::from_secs(9)),
        Step::AddLine {
            label: "Fill sequence #",
            value: Value::Number(4),
        },
        Step::AddLine {
            label: "Controller",
            value: Value::Boolean(false),
        },
        Step::AddLine {
            label: "Lower limit",
            value: Value::Number(CONTROLLER_LOWER_LIMIT),
        },
        Step::AddLine {
            label: "Upper limit",
            value: Value::Number(CONTROLLER_UPPER_LIMIT),
        },
        Step::Do(Command::BangBangInput { pin: FILL }),
        Step::Do(Command::BangBangOutput { pin: SET_PUMP }),
        Step::Do(Command::BangBangLowerLimit {
            value: CONTROLLER_LOWER_LIMIT,
        }),
        Step::Do(Command::BangBangUpperLimit {
            value: CONTROLLER_UPPER_LIMIT,
        }),
        // Step 5
        Step::At(Duration::from_secs(12)),
        Step::AddLine {
            label: "Fill sequence #",
            value: Value::Number(5),
        },
        Step::AddLine {
            label: "Controller",
            value: Value::Boolean(true),
        },
        Step::AddLine {
            label: "Pump",
            value: Value::Analog(PUMP),
        },
        Step::AddLine {
            label: "Fill level",
            value: Value::Analog(FILL),
        },
        Step::AddLine {
            label: "Source (NC)",
            value: Value::OffOn(SOURCE),
        },
        Step::Do(Command::BangBangStart),
        Step::Test {
            deadline: Duration::from_secs(20),
            condition: |measurements| measurements[FILL as usize] >= CONTROLLER_UPPER_LIMIT,
        },
        // Stwp 6
        Step::At(Duration::from_secs(20)),
        Step::AddLine {
            label: "Fill sequence #",
            value: Value::Number(6),
        },
        Step::AddLine {
            label: "Controller",
            value: Value::Boolean(false),
        },
        Step::AddLine {
            label: "Pump",
            value: Value::Analog(PUMP),
        },
        Step::AddLine {
            label: "Fill level",
            value: Value::Analog(FILL),
        },
        Step::AddLine {
            label: "Source (NC)",
            value: Value::OffOn(SOURCE),
        },
        Step::Do(Command::BangBangStop),
        // Step 7
        Step::At(Duration::from_secs(23)),
        Step::AddLine {
            label: "Fill sequence #",
            value: Value::Number(7),
        },
        Step::AddLine {
            label: "Controller",
            value: Value::Boolean(false),
        },
        Step::AddLine {
            label: "Pump",
            value: Value::Analog(PUMP),
        },
        Step::AddLine {
            label: "Fill level",
            value: Value::Analog(FILL),
        },
        Step::AddLine {
            label: "Source (NC)",
            value: Value::OffOn(SOURCE),
        },
        Step::Do(Command::SetAO {
            pin: SET_PUMP,
            value: 0,
        }),
        // Step 8
        Step::At(Duration::from_secs(26)),
        Step::AddLine {
            label: "Fill sequence #",
            value: Value::Number(8),
        },
        Step::AddLine {
            label: "Controller",
            value: Value::Boolean(false),
        },
        Step::AddLine {
            label: "Pump",
            value: Value::Analog(PUMP),
        },
        Step::AddLine {
            label: "Fill level",
            value: Value::Analog(FILL),
        },
        Step::Do(Command::SetDO {
            pin: SET_SOURCE,
            value: false,
        }),
        Step::AddLine {
            label: "Source (NC)",
            value: Value::OffOn(SOURCE),
        },
        Step::Test {
            deadline: Duration::from_secs(30),
            condition: |measurements| measurements[PUMP as usize] == 0,
        },
        // Step 9
        Step::At(Duration::from_secs(30)),
    ],
    abort: &[
        // Step 1
        Step::At(Duration::from_secs(0)),
        Step::AddLine {
            label: "Abort sequence #",
            value: Value::Number(1),
        },
        Step::AddLine {
            label: "Controller",
            value: Value::Boolean(false),
        },
        Step::AddLine {
            label: "Pump",
            value: Value::Analog(PUMP),
        },
        Step::AddLine {
            label: "Source (NC)",
            value: Value::OffOn(SOURCE),
        },
        Step::AddLine {
            label: "Drain (NO)",
            value: Value::OnOff(DRAIN),
        },
        Step::Do(Command::BangBangStop),
        Step::Do(Command::SetDO {
            pin: SET_DRAIN,
            value: false,
        }),
        Step::Test {
            deadline: Duration::from_secs(3),
            condition: |measurements| measurements[DRAIN as usize] == 0,
        },
        // Step 2
        Step::At(Duration::from_secs(3)),
        Step::AddLine {
            label: "Abort sequence #",
            value: Value::Number(2),
        },
        Step::AddLine {
            label: "Controller",
            value: Value::Boolean(false),
        },
        Step::AddLine {
            label: "Pump",
            value: Value::Analog(PUMP),
        },
        Step::AddLine {
            label: "Source (NC)",
            value: Value::OffOn(SOURCE),
        },
        Step::AddLine {
            label: "Drain (NO)",
            value: Value::OnOff(DRAIN),
        },
        Step::Do(Command::SetAO {
            pin: SET_PUMP,
            value: 0,
        }),
        Step::Test {
            deadline: Duration::from_secs(6),
            condition: |measurements| measurements[PUMP as usize] == 0,
        },
        // Step 3
        Step::At(Duration::from_secs(6)),
        Step::AddLine {
            label: "Abort sequence #",
            value: Value::Number(3),
        },
        Step::AddLine {
            label: "Controller",
            value: Value::Boolean(false),
        },
        Step::AddLine {
            label: "Pump",
            value: Value::Analog(PUMP),
        },
        Step::AddLine {
            label: "Source (NC)",
            value: Value::OffOn(SOURCE),
        },
        Step::AddLine {
            label: "Drain (NO)",
            value: Value::OnOff(DRAIN),
        },
        Step::Do(Command::SetDO {
            pin: SET_SOURCE,
            value: false,
        }),
        Step::Test {
            deadline: Duration::from_secs(9),
            condition: |measurements| measurements[SOURCE as usize] == 0,
        },
        // Step 4
        Step::At(Duration::from_secs(9)),
        Step::AddLine {
            label: "Abort sequence #",
            value: Value::Number(4),
        },
        Step::AddLine {
            label: "Controller",
            value: Value::Boolean(false),
        },
        Step::AddLine {
            label: "Pump",
            value: Value::Analog(PUMP),
        },
        Step::AddLine {
            label: "Source (NC)",
            value: Value::OffOn(SOURCE),
        },
        Step::AddLine {
            label: "Drain (NO)",
            value: Value::OnOff(DRAIN),
        },
        // Step 5
        Step::At(Duration::from_secs(12)),
    ],
}];
