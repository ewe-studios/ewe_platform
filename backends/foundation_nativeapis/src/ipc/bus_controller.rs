/// Top-level bus controller — manages the controller thread and endpoint lifecycle.
///
/// Implements the `join()` API with the `Rule` enum (Client/Server duality).

use std::{
    marker::PhantomData,
    sync::{Arc, RwLock},
    thread,
    time::{Duration, Instant},
};

use crate::ipc::{
    platform::{look_up, register, IoMultiplexing, EncodedMessage, IoHub, Remote},
    version::Version,
    util::EndpointID,
    Error, JoinError, Message, MessageBox, Options, RecvError, SendError,
};

/// Join a message bus.
pub fn join<T: MessageBox, R: MessageBox>(
    options: Options,
    timeout: Option<Duration>,
) -> Result<(EndpointSender<T>, EndpointReceiver<R>), JoinError> {
    let rule = Arc::new(RwLock::new(Rule::join(
        options,
        0,
        Arc::new(IoMultiplexing::new()),
        timeout,
    )?));

    Ok((
        EndpointSender {
            rule: rule.clone(),
            _marker: PhantomData,
        },
        EndpointReceiver {
            rule,
            _marker: PhantomData,
        },
    ))
}

/// The sending half of an endpoint. Cloneable.
pub struct EndpointSender<T> {
    rule: Arc<RwLock<Rule>>,
    _marker: PhantomData<T>,
}

impl<T> Clone for EndpointSender<T> {
    fn clone(&self) -> Self {
        Self {
            rule: self.rule.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T: MessageBox> EndpointSender<T> {
    pub fn send(&self, mut msg: Message<T>) -> Result<(), SendError> {
        msg.selector.memory_region_count = msg.memory_regions.len() as u16;
        let mut msg = msg.into_encoded();

        loop {
            let rule = self.rule.read().unwrap();
            match &*rule {
                Rule::Client { remote, epoch, .. } => {
                    match msg.send(remote) {
                        Err(Error::Disconnect) => {
                            let epoch = *epoch;
                            drop(rule);

                            let mut rule = self.rule.write().unwrap();
                            match &mut *rule {
                                Rule::Client {
                                    options,
                                    io_hub,
                                    reader_closed,
                                    im,
                                    epoch: epoch1,
                                    ..
                                } => {
                                    if epoch == *epoch1 {
                                        let reader_closed_val = *reader_closed;
                                        drop(io_hub.take());

                                        *rule = Rule::join(
                                            options.clone(),
                                            epoch.overflowing_add(1).0,
                                            im.clone(),
                                            None,
                                        )?;

                                        if reader_closed_val {
                                            rule.reader_close();
                                        }
                                    }
                                }
                                Rule::Server { .. } => {}
                            }
                        }
                        Err(_) => unreachable!(),
                        Ok(_) => break Ok(()),
                    }
                }
                Rule::Server {
                    bus_sender, im, ..
                } => {
                    bus_sender.lock().unwrap().send(msg).unwrap();
                    im.wake();
                    break Ok(());
                }
            }
        }
    }
}

/// The receiving half of an endpoint. Not cloneable.
pub struct EndpointReceiver<R> {
    rule: Arc<RwLock<Rule>>,
    _marker: PhantomData<R>,
}

impl<R> Drop for EndpointReceiver<R> {
    fn drop(&mut self) {
        let mut rule = self.rule.write().unwrap();
        rule.reader_close();
    }
}

impl<R: MessageBox> EndpointReceiver<R> {
    pub fn recv(&mut self, timeout: Option<Duration>) -> Result<Message<R>, RecvError> {
        loop {
            let rule = self.rule.read().unwrap();
            match &*rule {
                Rule::Client {
                    options,
                    remote,
                    io_hub,
                    reader_closed,
                    epoch,
                    ..
                } => {
                    if !*reader_closed && io_hub.is_none() {
                        let epoch_val = *epoch;
                        drop(rule);

                        let mut rule = self.rule.write().unwrap();
                        match &mut *rule {
                            Rule::Client {
                                options,
                                io_hub,
                                reader_closed,
                                im,
                                epoch: epoch1,
                                ..
                            } => {
                                if epoch_val == *epoch1 {
                                    let rc = *reader_closed;
                                    drop(io_hub.take());
                                    *rule = Rule::join(
                                        options.clone(),
                                        epoch_val.overflowing_add(1).0,
                                        im.clone(),
                                        timeout,
                                    )?;
                                    if rc { rule.reader_close(); }
                                }
                                continue;
                            }
                            Rule::Server { .. } => continue,
                        }
                    }

                    let mut io_hub_guard = io_hub.as_ref().expect("reader closed").lock().unwrap();

                    match io_hub_guard.recv(timeout, Some(remote)) {
                        Ok(encoded_msg) => {
                            if encoded_msg.selector.label_op.validate(&options.label) {
                                match R::decode(encoded_msg.selector.uuid, encoded_msg.payload_data)
                                {
                                    Ok(payload) => {
                                        let mut msg = Message::new(encoded_msg.selector, payload);
                                        msg.objects = encoded_msg.objects;
                                        msg.memory_regions = encoded_msg.memory_regions;
                                        break Ok(msg);
                                    }
                                    Err(Error::TypeUuidNotFound) => { continue; }
                                    Err(Error::Decode(err)) => { break Err(RecvError::Decode(err)); }
                                    Err(_) => unreachable!(),
                                }
                            } else { continue; }
                        }
                        Err(Error::Disconnect) => {
                            let epoch_val = *epoch;
                            drop(io_hub_guard);
                            drop(rule);

                            let mut rule = self.rule.write().unwrap();
                            match &mut *rule {
                                Rule::Client {
                                    options,
                                    io_hub,
                                    reader_closed,
                                    im,
                                    epoch: epoch1,
                                    ..
                                } => {
                                    if epoch_val == *epoch1 {
                                        let rc = *reader_closed;
                                        drop(io_hub.take());
                                        *rule = Rule::join(
                                            options.clone(),
                                            epoch_val.overflowing_add(1).0,
                                            im.clone(),
                                            timeout,
                                        )?;
                                        if rc { rule.reader_close(); }
                                    }
                                    continue;
                                }
                                Rule::Server { .. } => continue,
                            }
                        }
                        Err(Error::Timeout) => { break Err(RecvError::Timeout); }
                        Err(_) => unreachable!(),
                    }
                }
                Rule::Server { receiver, .. } => {
                    let receiver = receiver.as_ref().expect("reader closed").lock().unwrap();
                    break match timeout {
                        Some(timeout) => match receiver.recv_timeout(timeout) {
                            Ok(encoded_msg) => {
                                match R::decode(encoded_msg.selector.uuid, encoded_msg.payload_data) {
                                    Ok(payload) => {
                                        let mut msg = Message::new(encoded_msg.selector, payload);
                                        msg.objects = encoded_msg.objects;
                                        msg.memory_regions = encoded_msg.memory_regions;
                                        Ok(msg)
                                    }
                                    Err(Error::TypeUuidNotFound) => { continue; }
                                    Err(Error::Decode(err)) => Err(RecvError::Decode(err)),
                                    Err(_) => unreachable!(),
                                }
                            }
                            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err(RecvError::Timeout),
                            Err(_) => unreachable!(),
                        },
                        None => {
                            let encoded_msg = receiver.recv().unwrap();
                            match R::decode(encoded_msg.selector.uuid, encoded_msg.payload_data) {
                                Ok(payload) => {
                                    let mut msg = Message::new(encoded_msg.selector, payload);
                                    msg.objects = encoded_msg.objects;
                                    msg.memory_regions = encoded_msg.memory_regions;
                                    Ok(msg)
                                }
                                Err(Error::TypeUuidNotFound) => { continue; }
                                Err(Error::Decode(err)) => Err(RecvError::Decode(err)),
                                Err(_) => unreachable!(),
                            }
                        }
                    };
                }
            }
        }
    }
}

enum Rule {
    Client {
        endpoint_id: EndpointID,
        options: Options,
        remote: Remote,
        io_hub: Option<std::sync::Mutex<IoHub>>,
        reader_closed: bool,
        im: Arc<IoMultiplexing>,
        epoch: u32,
    },
    Server {
        endpoint_id: EndpointID,
        bus_sender: std::sync::Mutex<std::sync::mpsc::Sender<EncodedMessage>>,
        receiver: Option<std::sync::Mutex<std::sync::mpsc::Receiver<EncodedMessage>>>,
        im: Arc<IoMultiplexing>,
    },
}

impl Rule {
    fn join(
        options: Options,
        epoch: u32,
        im: Arc<IoMultiplexing>,
        timeout: Option<Duration>,
    ) -> Result<Self, JoinError> {
        let end = timeout.map(|t| Instant::now() + t);

        macro_rules! wait {
            () => {
                let mut w = Duration::from_secs(2);
                if let Some(end) = end {
                    let remain = end.saturating_duration_since(Instant::now());
                    if remain.is_zero() { return Err(JoinError::Timeout); }
                    w = w.min(remain);
                }
                thread::sleep(w);
            };
        }

        let mut timeout_count = 0;
        let mut perm_denied = 0;

        let rule = loop {
            let r = look_up(
                &options.identifier,
                options.label.clone(),
                options.token.clone(),
                im.clone(),
            );

            match r {
                Ok((io_hub, remote, eid)) => {
                    break Rule::Client {
                        endpoint_id: eid,
                        options,
                        remote,
                        io_hub: Some(std::sync::Mutex::new(io_hub)),
                        reader_closed: false,
                        im,
                        epoch,
                    };
                }
                Err(Error::IdentifierNotInUse) => {
                    if !options.controller_affinity { wait!(); continue; }

                    let r = register(&options.identifier, im.clone());
                    match r {
                        Ok((io_hub, bus_sender, eid)) => {
                            let (tx, rx) = std::sync::mpsc::channel();
                            let im2 = io_hub.io_multiplexing();

                            let ctrl = super::platform::BusController::new(
                                eid, options.label, options.token, tx, io_hub,
                            );
                            ctrl.run();

                            break Rule::Server {
                                endpoint_id: eid,
                                bus_sender: std::sync::Mutex::new(bus_sender),
                                receiver: Some(std::sync::Mutex::new(rx)),
                                im: im2,
                            };
                        }
                        Err(Error::IdentifierInUse) => {}
                        Err(Error::PermissionDenied) => {
                            perm_denied += 1;
                            if perm_denied > 5 { return Err(JoinError::PermissionDenied); }
                            wait!();
                        }
                        Err(e) => { tracing::error!("register: {:?}", e); wait!(); }
                    }
                }
                Err(Error::VersionMismatch(v, _)) => {
                    return Err(JoinError::VersionMismatch(v));
                }
                Err(Error::TokenMismatch) => {
                    return Err(JoinError::TokenMismatch);
                }
                Err(Error::PermissionDenied) => {
                    perm_denied += 1;
                    if perm_denied > 5 { return Err(JoinError::PermissionDenied); }
                    wait!();
                }
                Err(Error::Timeout) => {
                    timeout_count += 1;
                    if timeout_count > 5 {
                        return Err(JoinError::VersionMismatch(Version::new()));
                    }
                    wait!();
                }
                Err(e) => { tracing::error!("look_up: {:?}", e); wait!(); }
            }
        };

        Ok(rule)
    }
}

impl Rule {
    fn reader_close(&mut self) {
        match self {
            Rule::Client { io_hub, reader_closed, .. } => {
                let _ = io_hub.take();
                *reader_closed = true;
            }
            Rule::Server { receiver, .. } => {
                let _ = receiver.take();
            }
        }
    }
}
