use std::error::Error;
use std::net::Ipv4Addr;
use std::sync::{
    mpsc::{self, Sender},
    Arc, Mutex,
};
use std::thread;

use actix_web::web::Data;
use log::{error, info};

use crate::models::{Light, LightRequest, LightingResponse, Payload};
use crate::Storage;

enum DispatchMessage {
    Job((Ipv4Addr, LightRequest, Sender<ReplyMessage>)),
    Shutdown,
}

enum ReplyMessage {
    Reply(LightingResponse),
    Shutdown,
}

/// Threadpool manager for dispatching worker tasks and managing reply state
pub struct Worker {
    tx: Sender<DispatchMessage>,
    reply_tx: Sender<ReplyMessage>,
    thread: Option<thread::JoinHandle<()>>,
    reply_thread: Option<thread::JoinHandle<()>>,
}

fn send_reply(resp: Result<LightingResponse, Box<dyn Error>>, tx: Sender<ReplyMessage>) {
    match resp {
        Ok(resp) => {
            if let Err(e) = tx.send(ReplyMessage::Reply(resp)) {
                error!("Failed to sync response: {:?}", e);
            }
        }
        Err(e) => {
            error!("Lighting error: {}", e);
        }
    };
}

fn handle_request(ip: Ipv4Addr, request: LightRequest, tx: Sender<ReplyMessage>) {
    let light = Light::new(ip, None);
    let payload = Payload::from(&request);
    if payload.is_valid() {
        send_reply(light.set(&payload), tx.clone());
    }
    if let Some(power) = request.power() {
        send_reply(light.set_power(power), tx);
    }
}

impl Worker {
    /// Create a new [Worker] dispatch (this should only happen once)
    ///
    /// Provide a clone of the [Data] & [Mutex] wrapped [Storage] object
    ///
    pub fn new(data: Data<Mutex<Storage>>) -> Self {
        let (tx, rx) = mpsc::channel::<DispatchMessage>();
        let (reply_tx, reply_rx) = mpsc::channel::<ReplyMessage>();
        let pool = ThreadPool::new(4);

        let handle = thread::spawn(move || {
            for msg in rx {
                match msg {
                    DispatchMessage::Job(msg) => {
                        pool.execute(move || {
                            handle_request(msg.0, msg.1, msg.2);
                        });
                    }
                    DispatchMessage::Shutdown => {
                        return;
                    }
                }
            }
        });

        let reply_handle = thread::spawn(move || {
            for msg in reply_rx {
                match msg {
                    ReplyMessage::Reply(resp) => {
                        let mut data = data.lock().unwrap();
                        data.process_reply(&resp);
                    }
                    ReplyMessage::Shutdown => {
                        return;
                    }
                }
            }
        });

        Worker {
            tx,
            reply_tx,
            thread: Some(handle),
            reply_thread: Some(reply_handle),
        }
    }

    /// Queue a lighting setting change for the light by IP
    ///
    /// The work will be executed in the next available thread
    ///
    pub fn create_task(&mut self, ip: Ipv4Addr, req: LightRequest) -> Result<(), Box<dyn Error>> {
        self.tx
            .send(DispatchMessage::Job((ip, req, self.reply_tx.clone())))?;
        Ok(())
    }

    /// Queue an update from a lighting setting change
    ///
    /// This is the reply path from [Self::create_task]
    ///
    /// This will alert the dispatch they need to take the [Storage]
    /// [Data] [Mutex] to write the response to the affected
    /// [Light] and update `rooms.json`
    ///
    pub fn queue_update(&mut self, resp: LightingResponse) -> Result<(), Box<dyn Error>> {
        self.reply_tx.send(ReplyMessage::Reply(resp))?;
        Ok(())
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        info!("shutting down dispatch");
        if let Err(e) = self.tx.send(DispatchMessage::Shutdown) {
            error!("Failed to send dispatch shutdown: {}", e);
        }

        if let Some(thread) = self.thread.take() {
            thread.join().unwrap_or_else(|_| {
                error!("failed to shutdown dispatch");
            });
        }

        if let Err(e) = self.reply_tx.send(ReplyMessage::Shutdown) {
            error!("Failed to send response listener shutdown: {}", e);
        }

        if let Some(thread) = self.reply_thread.take() {
            thread.join().unwrap_or_else(|_| {
                error!("failed to shutdown response listener");
            });
        }
    }
}

trait FnBox {
    fn call_box(self: Box<Self>);
}

impl<F: FnOnce()> FnBox for F {
    fn call_box(self: Box<F>) {
        (*self)()
    }
}

enum Message {
    Job(Box<dyn FnBox + Send + 'static>),
    Shutdown,
}

struct ThreadPool {
    runners: Vec<Runner>,
    sender: Sender<Message>,
}

impl ThreadPool {
    /// Create a new ThreadPool.
    ///
    /// The size is the number of threads in the pool.
    ///
    /// # Panics
    ///
    /// The `new` function will panic if the size is zero.
    pub fn new(size: usize) -> ThreadPool {
        assert!(size > 0); // return a Result type if this is recoverable

        let (sender, receiver) = mpsc::channel();

        let receiver = Arc::new(Mutex::new(receiver));

        let mut runners = Vec::with_capacity(size);

        for id in 0..size {
            runners.push(Runner::new(id, Arc::clone(&receiver)));
        }

        ThreadPool { runners, sender }
    }

    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.sender.send(Message::Job(Box::new(f))).unwrap();
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        info!("shutting down runners");
        for _ in &mut self.runners {
            self.sender.send(Message::Shutdown).unwrap();
        }

        for runner in &mut self.runners {
            if let Some(thread) = runner.thread.take() {
                thread.join().unwrap();
            }
        }
    }
}

struct Runner {
    thread: Option<thread::JoinHandle<()>>,
}

impl Runner {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Message>>>) -> Self {
        let thread = thread::spawn(move || loop {
            let job = receiver.lock().unwrap().recv().unwrap();
            match job {
                Message::Job(j) => {
                    j.call_box();
                }
                Message::Shutdown => {
                    info!("runner {id} shutting down");
                    return;
                }
            }
        });

        Runner {
            thread: Some(thread),
        }
    }
}
