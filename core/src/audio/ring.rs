//! A lock-free queue from the game thread to the audio thread.
//!
//! # Why this file exists at all
//!
//! The obvious way to share state between a game and its mixer is a
//! `Mutex`. It is also the classic way to ship audio that is perfect on
//! the developer's machine and crackles on someone else's: the game
//! thread takes the lock, gets descheduled or hits a slow frame, and the
//! audio callback — which has about 5 ms and no right to block — waits
//! behind it and misses its deadline. The symptom is a click, it is
//! load-dependent, and it is close to unreproducible.
//!
//! So the two threads never share a lock. Commands go one way through
//! this ring; the only thing coming back is an atomic counter.
//!
//! # The shape
//!
//! Single producer (the game thread), single consumer (the audio
//! thread), fixed capacity, no allocation after construction. That is
//! the weakest structure that does the job, and weak is what we want
//! here: this is the only `unsafe` in the audio layer, and every feature
//! it does not have is a bug it cannot have.

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use super::params::Command;

/// How many commands may be in flight.
///
/// A frame at 60 fps issues a handful — three `set`s and a few `play`s
/// in the racer's worst case. 256 is two orders of magnitude of slack,
/// and 8 KiB of it, so the size is not worth tuning.
pub(crate) const CAPACITY: usize = 256;

/// The queue itself.
///
/// `head` is only ever advanced by the consumer, `tail` only by the
/// producer. That single-writer-per-index rule is what makes the
/// `UnsafeCell` sound, and it is why this type must never grow a second
/// producer.
pub(crate) struct Ring {
    slots: [UnsafeCell<Command>; CAPACITY],
    /// Next slot to read. Written by the consumer only.
    head: AtomicUsize,
    /// Next slot to write. Written by the producer only.
    tail: AtomicUsize,
    /// Commands discarded because the ring was full. Diagnostic only.
    dropped: AtomicU32,
}

// SAFETY: `Command` is `Copy` and has no destructor (asserted in
// `params::tests::params_are_plain_data`). Each slot is written only by
// the producer, and only at an index the consumer has already passed;
// each slot is read only by the consumer, and only at an index the
// producer has already published via `tail`. The `Release`/`Acquire`
// pair on `tail` is what orders the slot write before the slot read.
unsafe impl Sync for Ring {}
unsafe impl Send for Ring {}

impl Ring {
    pub(crate) fn new() -> Ring {
        Ring {
            slots: std::array::from_fn(|_| UnsafeCell::new(Command::Master { gain: 1.0 })),
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            dropped: AtomicU32::new(0),
        }
    }

    /// Enqueue a command. **Game thread only.** Never blocks.
    ///
    /// When the ring is full the command is dropped and a counter
    /// increments. That is deliberate: the alternatives are to block the
    /// game thread (a dropped frame, to protect a sound effect) or to
    /// grow the buffer (an allocation, on a path that must not
    /// allocate). A lost whoosh is the cheapest of the three, and a
    /// full ring means something upstream is already wrong.
    pub(crate) fn push(&self, cmd: Command) {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);

        if tail.wrapping_sub(head) >= CAPACITY {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return;
        }

        // SAFETY: `tail - head < CAPACITY`, so this slot is not one the
        // consumer can be reading: it has not been published yet.
        unsafe {
            *self.slots[tail % CAPACITY].get() = cmd;
        }

        // Release: everything written above — including the slot itself —
        // is visible to any thread that reads this `tail` with Acquire.
        // Publishing the index is what makes the payload visible, so
        // this store must come last and must not be Relaxed.
        self.tail.store(tail.wrapping_add(1), Ordering::Release);
    }

    /// Dequeue one command. **Audio thread only.** Never blocks.
    pub(crate) fn pop(&self) -> Option<Command> {
        let head = self.head.load(Ordering::Relaxed);
        // Acquire, pairing with the producer's Release above: if we see
        // this `tail`, we also see the slot contents written before it.
        let tail = self.tail.load(Ordering::Acquire);

        if head == tail {
            return None;
        }

        // SAFETY: `head != tail`, so this slot was published by the
        // producer and will not be written again until we advance `head`.
        let cmd = unsafe { *self.slots[head % CAPACITY].get() };

        // Release: tells the producer this slot is free for reuse only
        // after we have finished reading it.
        self.head.store(head.wrapping_add(1), Ordering::Release);
        Some(cmd)
    }

    /// Commands lost to a full ring, since the start of the run.
    pub(crate) fn dropped(&self) -> u32 {
        self.dropped.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::params::{VoiceId, VoiceParams};

    fn set(n: u16) -> Command {
        Command::Set { voice: VoiceId(n), params: VoiceParams::engine(0.5) }
    }

    fn voice_of(cmd: Command) -> u16 {
        match cmd {
            Command::Set { voice, .. } => voice.0,
            other => panic!("expected Set, got {other:?}"),
        }
    }

    #[test]
    fn commands_come_out_in_the_order_they_went_in() {
        let r = Ring::new();
        for i in 0..8 {
            r.push(set(i));
        }
        for i in 0..8 {
            assert_eq!(voice_of(r.pop().expect("queued")), i);
        }
        assert!(r.pop().is_none(), "the ring should be empty");
    }

    #[test]
    fn a_full_ring_drops_rather_than_blocking_or_growing() {
        let r = Ring::new();
        for i in 0..CAPACITY {
            r.push(set(i as u16));
        }
        assert_eq!(r.dropped(), 0, "nothing should be dropped while there is room");

        r.push(set(9999));
        assert_eq!(r.dropped(), 1, "the overflowing command should be counted");

        // The queue still holds exactly what it accepted, in order — a
        // full ring must not corrupt what is already in it.
        for i in 0..CAPACITY {
            assert_eq!(voice_of(r.pop().expect("queued")), i as u16);
        }
        assert!(r.pop().is_none());
    }

    #[test]
    fn indices_wrap_without_losing_or_reordering_anything() {
        // Push and pop far more than CAPACITY so `head` and `tail` wrap
        // past the modulus many times. A ring that only works below its
        // capacity is a ring that fails after a few minutes of play.
        let r = Ring::new();
        let mut expect = 0u16;
        for round in 0..40u16 {
            for i in 0..17u16 {
                r.push(set(round.wrapping_mul(17).wrapping_add(i)));
            }
            for _ in 0..17 {
                assert_eq!(voice_of(r.pop().expect("queued")), expect);
                expect = expect.wrapping_add(1);
            }
        }
        assert_eq!(r.dropped(), 0);
    }

    #[test]
    fn a_producer_and_a_consumer_on_real_threads_lose_nothing() {
        // The ordering claims in `push`/`pop` are about two threads, so
        // one thread cannot test them. This will not *prove* the
        // ordering correct, but it does exercise it under a real race,
        // and it fails loudly if a slot is read before it is published.
        use std::sync::Arc;

        const N: usize = 20_000;
        let r = Arc::new(Ring::new());
        let producer = {
            let r = Arc::clone(&r);
            std::thread::spawn(move || {
                let mut sent = 0usize;
                while sent < N {
                    let before = r.dropped();
                    r.push(set((sent % 60_000) as u16));
                    if r.dropped() == before {
                        sent += 1;
                    } else {
                        std::thread::yield_now();
                    }
                }
            })
        };

        let mut got = 0usize;
        while got < N {
            if let Some(cmd) = r.pop() {
                assert_eq!(voice_of(cmd), (got % 60_000) as u16, "out of order at {got}");
                got += 1;
            } else {
                std::thread::yield_now();
            }
        }
        producer.join().expect("producer panicked");
        assert_eq!(got, N);
    }
}
