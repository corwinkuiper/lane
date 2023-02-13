use core::{
    future::{poll_fn, Future},
    pin::Pin,
    ptr,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

use agb::InternalAllocator;

use alloc::boxed::Box;

enum Either<L, R> {
    Left(L),
    Right(R),
}

struct Executor<O> {
    waker: Waker,
    fut: Pin<Box<dyn Future<Output = O>, InternalAllocator>>,
}

pub struct Evaluator<O> {
    data: Either<Executor<O>, O>,
}

impl<O> Evaluator<O> {
    pub fn new(future: impl Future<Output = O> + 'static) -> Self {
        fn make_waker() -> Waker {
            unsafe { Waker::from_raw(RawWaker::new(ptr::null(), &NOOP_VTABLE)) }
        }

        let waker = make_waker();

        Self {
            data: Either::Left(Executor {
                fut: Box::pin_in(future, InternalAllocator),
                waker,
            }),
        }
    }

    pub fn do_work(&mut self) -> Option<&O> {
        match &mut self.data {
            Either::Left(exe) => {
                match exe.fut.as_mut().poll(&mut Context::from_waker(&exe.waker)) {
                    core::task::Poll::Ready(result) => self.data = Either::Right(result),
                    core::task::Poll::Pending => {}
                };
            }
            Either::Right(_) => {}
        }

        self.result()
    }

    pub fn result(&self) -> Option<&O> {
        match &self.data {
            Either::Left(_) => None,
            Either::Right(o) => Some(o),
        }
    }
}

const NOOP_VTABLE: RawWakerVTable = RawWakerVTable::new(noop_clone, noop, noop, noop);

fn noop_clone(data: *const ()) -> RawWaker {
    RawWaker::new(data, &NOOP_VTABLE)
}

fn noop(data: *const ()) {}

pub fn yeild() -> impl Future<Output = ()> {
    let mut done = false;
    poll_fn(move |_ctx| {
        if !done {
            done = true;
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    })
}
