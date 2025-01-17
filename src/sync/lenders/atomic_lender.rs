use std::{
    mem,
    ops::DerefMut,
    sync::{Arc, Condvar, Mutex},
};

#[derive(Debug)]
pub struct AtomicLender<Inner> {
    state: Mutex<State<Inner>>,
    condvar: Condvar,
}

#[derive(Debug)]
enum State<Inner> {
    Available(Inner),
    Lent,
}

impl<Inner> AtomicLender<Inner> {
    pub fn new(inner: Inner) -> Self {
        AtomicLender {
            state: Mutex::new(State::Available(inner)),
            condvar: Condvar::new(),
        }
    }

    pub fn take(self: &Arc<Self>) -> Inner {
        let guard = self.state.lock().unwrap();
        let mut guard = (*self)
            .condvar
            .wait_while(guard, |state| match state {
                State::Available(_) => false,
                State::Lent => true,
            })
            .unwrap();

        let mut state = State::Lent;
        mem::swap(guard.deref_mut(), &mut state);

        match state {
            State::Available(inner) => inner,
            State::Lent => unreachable!(),
        }
    }

    pub fn try_take(self: &Arc<Self>) -> Option<Inner> {
        let mut guard = self.state.lock().unwrap();

        let mut state = State::Lent;
        mem::swap(guard.deref_mut(), &mut state);

        match state {
            State::Available(inner) => Some(inner),
            State::Lent => None,
        }
    }

    pub fn restore(self: &Arc<Self>, inner: Inner) {
        let mut guard = self.state.lock().unwrap();
        if let State::Lent = *guard {
            *guard = State::Available(inner);
        } else {
            panic!(
                "attempted to `AtomicLender::restore` more than once without `AtomicLender::take`"
            );
        }

        self.condvar.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, thread::JoinHandle, time::Duration};

    #[test]
    fn stress() {
        let lender = Arc::new(AtomicLender::new(1));

        let threads: Vec<JoinHandle<()>> = (0..32)
            .map(|index| {
                let lender = lender.clone();
                thread::spawn(move || {
                    if index < 16 {
                        for _ in 0..10 {
                            let thing = lender.take();
                            thread::sleep(Duration::from_millis(1));
                            lender.restore(thing);
                        }
                    } else {
                        for _ in 0..10 {
                            if let Some(thing) = lender.try_take() {
                                thread::sleep(Duration::from_millis(1));
                                lender.restore(thing);
                            }
                        }
                    }
                })
            })
            .collect();

        for thread in threads {
            thread.join().unwrap();
        }
    }
}
