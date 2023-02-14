use conquer_once::spin::OnceCell;
use core::{
    pin::Pin,
    task::{Poll, Context},
};
use crate::{print, println};
use crossbeam_queue::ArrayQueue;
use futures_util::{
    stream::StreamExt,
    Stream,
    task::AtomicWaker,
};
use pc_keyboard::{
    layouts,
    DecodedKey,
    HandleControl,
    Keyboard,
    ScancodeSet1,
};

static SCANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();
static WAKER: AtomicWaker = AtomicWaker::new();

/// Called by the keyboard interrupt handler
/// 
/// Must not block or allocate
pub(crate) fn add_scancode(scancode: u8) {
    if let Ok(q) = SCANCODE_QUEUE.try_get() {
        if let Err(_) = q.push(scancode) {
            println!("WARNING: scancode queue full, dropping keyboard input.");
        } else {
            WAKER.wake();
        }
    } else {
        println!("WARNING: scancode queue uninitialized.");
    }
}

pub async fn print_keypresses() {
    let mut scancodes = ScancodeStream::new();
    let mut keyboard = Keyboard::new(layouts::Us104Key, ScancodeSet1, HandleControl::Ignore);

    while let Some(scancode) = scancodes.next().await {
        if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
            if let Some(key) = keyboard.process_keyevent(key_event) {
                match key {
                    DecodedKey::Unicode(c) => print!("{c}"),
                    DecodedKey::RawKey(k) => print!("{:?}", k),
                }
            }
        }
    }
}

pub struct ScancodeStream {
    _private: (),
}

impl Stream for ScancodeStream {
    type Item = u8;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let q = SCANCODE_QUEUE.try_get().expect("not initialized");

        if let Ok(scancode) = q.pop() {
            return Poll::Ready(Some(scancode));
        }

        WAKER.register(&cx.waker());
        match q.pop() {
            Ok(scancode) => {
                WAKER.take();
                Poll::Ready(Some(scancode))
            }
            Err(crossbeam_queue::PopError) => Poll::Pending,
        }
    }
}

impl ScancodeStream {
    pub fn new() -> Self {
        SCANCODE_QUEUE.try_init_once(|| ArrayQueue::new(100))
            .expect("ScancodeStream::new should only be called once");
        ScancodeStream { _private: () }
    }
}