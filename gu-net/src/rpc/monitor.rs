use rand::*;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct MonitorConfig {
    pub max_wait_time: Duration,
    pub max_no_interaction_time: Duration,
}

impl MonitorConfig {
    pub fn monitor(&self) -> Monitor {
        Monitor {
            last_interaction: None,
            pending_pong: None,
            next_wait_time: Duration::default(),
            configuration: self.clone(),
        }
    }
}

impl Default for MonitorConfig {
    fn default() -> Self {
        MonitorConfig {
            max_wait_time: Duration::from_secs(10),
            max_no_interaction_time: Duration::from_secs(15),
        }
    }
}

pub struct Monitor {
    last_interaction: Option<Instant>,
    pending_pong: Option<(String, Instant)>,
    next_wait_time: Duration,
    configuration: MonitorConfig,
}

pub enum MonitorAction {
    Continue,
    SendPing(String),
    Stop,
}

impl Monitor {
    fn need_interaction(&self) -> bool {
        match self.last_interaction {
            Some(ts) => {
                Instant::now().duration_since(ts) > self.configuration.max_no_interaction_time
            }
            None => true,
        }
    }

    pub fn interaction(&mut self) {
        self.last_interaction = Some(Instant::now())
    }

    pub fn pong(&mut self, text: &str) {
        let is_valid = match &self.pending_pong {
            Some((ping_text, _)) => ping_text == text,
            None => false,
        };
        if is_valid {
            self.pending_pong = None;
            self.last_interaction = Some(Instant::now())
        }
    }

    pub fn next_action(&mut self) -> MonitorAction {
        let now = Instant::now();

        match self.pending_pong.take() {
            Some((s, ts)) => if now.duration_since(ts) > self.configuration.max_wait_time {
                return MonitorAction::Stop;
            } else {
                return MonitorAction::Continue;
            },
            None => (),
        }

        if self.need_interaction() {
            let rnd_val: [u64; 2] = thread_rng().gen();
            let ping_text = format!("{:x}{:x}", rnd_val[0], rnd_val[1]);

            self.pending_pong = Some((ping_text.clone(), now));
            return MonitorAction::SendPing(ping_text);
        }

        return MonitorAction::Continue;
    }
}
