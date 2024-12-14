use std::{
    iter::FusedIterator,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    thread::{self, JoinHandle},
};

#[derive(Debug)]
pub struct Generator<T> {
    next_count: Arc<AtomicU64>,
    receiver: Receiver<T>,
    coroutine: Option<JoinHandle<()>>,
}

impl<T: Send + 'static> Generator<T> {
    pub fn new(func: impl FnOnce(Yielder<T>) + Send + 'static) -> Self {
        let next_count = Arc::new(AtomicU64::new(0));
        let (sender, receiver) = mpsc::channel();
        let coroutine = {
            let yielder = Yielder {
                next_count: Arc::clone(&next_count),
                sender,
            };
            thread::spawn(|| func(yielder))
        };

        Self {
            next_count,
            receiver,
            coroutine: Some(coroutine),
        }
    }
}

impl<T> Iterator for Generator<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let coroutine = self.coroutine.as_ref()?;

        if coroutine.is_finished() {
            self.coroutine.take().unwrap().join().unwrap();
            return None;
        }

        self.next_count.fetch_add(1, Ordering::AcqRel);

        self.receiver.recv().ok()
    }
}

impl<T> FusedIterator for Generator<T> {}

pub struct Yielder<T> {
    next_count: Arc<AtomicU64>,
    sender: Sender<T>,
}

impl<T> Yielder<T> {
    pub fn r#yield(&self, value: T) {
        while self.next_count.load(Ordering::Acquire) == 0 {
            thread::yield_now();
        }

        self.next_count.fetch_sub(1, Ordering::AcqRel);
        self.sender.send(value).unwrap();
    }

    pub fn yield_from<I: Iterator<Item = T>>(&self, iter: I) {
        for i in iter {
            self.r#yield(i);
        }
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn iteration() {
        let mut gen = Generator::new(|y| {
            y.r#yield(1);
            y.r#yield(2);
            y.r#yield(3);
        });

        assert_eq!(gen.next(), Some(1));
        assert_eq!(gen.next(), Some(2));
        assert_eq!(gen.next(), Some(3));
        assert_eq!(gen.next(), None);
    }

    #[test]
    fn for_loop() {
        let gen = Generator::new(|y| {
            y.r#yield(1);
            y.r#yield(2);
            y.r#yield(3);
        });

        let mut returns = [1, 2, 3].into_iter();
        for i in gen {
            assert_eq!(i, returns.next().unwrap());
        }
    }

    #[test]
    fn big_generator() {
        let gen = Generator::new(|y| {
            for i in 0..10000 {
                y.r#yield(i);
            }
        });

        assert!(gen.eq(0..10000))
    }

    #[test]
    fn yield_from() {
        let gen = Generator::new(|y| {
            y.yield_from(Generator::new(|y2| {
                for i in 0..100 {
                    y2.r#yield(i);
                }
            }));

            y.yield_from(0..100);
        });

        assert!(gen.eq((0..100).chain(0..100)))
    }
}
