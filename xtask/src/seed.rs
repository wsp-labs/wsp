// SPDX-License-Identifier: AGPL-3.0-only
//! The order a replay sends its frames in. Sequential waits for each reply before the next frame; seeded sends a
//! window of one to four frames before reading, the window drawn from one seed per tape, so an interleaving that
//! breaks a host comes back with the same seed.

/// SplitMix64: one u64 of state, the same stream on every computer for the same seed.
#[derive(Clone, Debug)]
pub struct Order {
    state: Option<u64>,
}

impl Order {
    pub fn sequential() -> Order {
        Order { state: None }
    }

    pub fn seeded(seed: u64) -> Order {
        Order { state: Some(seed) }
    }

    /// The seed one tape replays under: the run's seed mixed with the tape's id, so a tape's order does not depend
    /// on which other tapes ran or in what order the workers took them.
    pub fn for_tape(seed: Option<u64>, tape: &str) -> Order {
        match seed {
            None => Order::sequential(),
            Some(seed) => Order::seeded(seed ^ fnv(tape)),
        }
    }

    pub fn is_seeded(&self) -> bool {
        self.state.is_some()
    }

    /// How many frames go out before the replay reads their replies.
    pub fn window(&mut self) -> usize {
        match self.next() {
            None => 1,
            Some(n) => 1 + (n % 4) as usize,
        }
    }

    fn next(&mut self) -> Option<u64> {
        let s = self.state.as_mut()?;
        *s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = *s;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        Some(z ^ (z >> 31))
    }
}

fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
}

/// A seed for a run that names none, read off the clock, printed so the run can be repeated.
pub fn fresh_seed() -> u64 {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0);
    let mut o = Order::seeded(nanos ^ u64::from(std::process::id()));
    o.next().unwrap_or(1)
}
