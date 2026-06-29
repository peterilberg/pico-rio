use core::cell::RefCell;
use embassy_time::{Duration, Instant, Timer};
use messages::{Command, Label};
use {defmt_rtt as _, panic_probe as _};

use super::{Guard, Sequence, Step};
use crate::dispatcher;
use crate::display;
use crate::measurements::Measurements;

pub struct Execution<'seq> {
    state: RefCell<State<'seq>>,
}

enum State<'seq> {
    NotRunning,
    // TODO depending on how it goes, split into Aborting state
    Running {
        sequence: &'seq Sequence<'seq>,
        step: usize,
        start_time: Instant,
        is_aborting: bool, // TODO reason: guard / test + step
    },
}

impl<'seq> Execution<'seq> {
    pub fn new() -> Self {
        Execution {
            state: RefCell::new(State::NotRunning),
        }
    }

    pub async fn start(&self, sequence: &'seq Sequence<'seq>) {
        let mut state = self.state.borrow_mut();
        (*state).start(sequence).await;
    }

    pub async fn stop(&self) {
        let mut state = self.state.borrow_mut();
        match *state {
            State::NotRunning => {}
            State::Running {
                is_aborting: false, ..
            } => {
                display::remove_page().await;
                (*state).stop();
            }
            State::Running {
                is_aborting: true,
                sequence,
                ..
            } => {
                log::info!("sequencer: running abort sequence {}", sequence.id);
            }
        }
    }

    pub async fn execute_step(&self, measurements: &Measurements) {
        let mut state = self.state.borrow_mut();
        match *state {
            State::NotRunning => {
                Timer::after(Duration::from_secs(10)).await;
            }
            State::Running {
                sequence,
                step,
                is_aborting,
                start_time,
            } => {
                log::info!("sequencer: STEPPING");
                // TODO check guards
                if !is_aborting && let Some(guard) = self.get_failing_guard(sequence, measurements)
                {
                    log::info!("sequencer: guard {} failed", guard.id);
                    (*state).abort();
                    return;
                }

                // TODO get next state
                let steps = if is_aborting {
                    sequence.abort
                } else {
                    sequence.steps
                };
                let Some(step_to_execute) = steps.get(step) else {
                    dispatcher::dispatch(Command::RemovePage).await;
                    (*state).stop();
                    return;
                };

                // TODO dispatch
                match step_to_execute {
                    Step::At(deadline) => {
                        Timer::at(start_time + *deadline).await;
                        (*state).advance();
                        dispatcher::dispatch(Command::ClearDisplay).await;
                    }
                    Step::Test { condition, .. } if condition(measurements) => {
                        (*state).advance();
                    }
                    Step::Test { deadline, .. } if !is_aborting => {
                        Timer::at(start_time + *deadline).await;
                        (*state).abort();
                    }
                    Step::Test { deadline, .. } => {
                        Timer::at(start_time + *deadline).await;
                        (*state).advance();
                    }
                    Step::Do(command) => {
                        dispatcher::dispatch((*command).clone()).await;
                        (*state).advance();
                    }
                    Step::AddLine { label, value } => {
                        let mut string = Label::new();
                        if let Ok(()) = string.push_str(*label) {
                            dispatcher::dispatch(Command::AddLine {
                                label: string,
                                value: (*value).clone(),
                            })
                            .await;
                        }
                        (*state).advance();
                    }
                }
            }
        }
    }

    fn get_failing_guard(
        &self,
        sequence: &Sequence<'seq>,
        measurements: &Measurements,
    ) -> Option<&'seq Guard> {
        sequence
            .guards
            .iter()
            .find(|guard| (guard.condition)(&measurements))
    }
}

impl<'seq> State<'seq> {
    async fn start(&mut self, sequence: &'seq Sequence<'seq>) {
        match self {
            State::NotRunning => {
                dispatcher::dispatch(Command::AddPage).await;
                *self = Self::new(sequence, false);
            }
            State::Running { sequence, .. } => {
                log::info!("sequencer: already running sequence {}", sequence.id);
            }
        }
    }

    fn new(sequence: &'seq Sequence<'seq>, is_aborting: bool) -> Self {
        State::Running {
            sequence,
            step: 0,
            start_time: Instant::now(),
            is_aborting,
        }
    }

    fn stop(&mut self) {
        *self = State::NotRunning;
    }

    fn abort(&mut self) {
        if let State::Running { sequence, .. } = self {
            *self = State::Running {
                sequence,
                step: 0,
                start_time: Instant::now(),
                is_aborting: true,
            };
        }
    }

    fn advance(&mut self) {
        if let State::Running { step, .. } = self {
            *step += 1;
        }
    }
}
