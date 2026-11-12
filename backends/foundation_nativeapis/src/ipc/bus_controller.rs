/// Top-level bus controller — manages the controller thread and endpoint lifecycle.
///
/// Implements the `join()` API with the `Rule` enum (Client/Server duality).
use std::{
    marker::PhantomData,
    sync::{Arc, RwLock},
    thread,
    time::Duration,
};

use foundation_context::Ctx;

use crate::ipc::{
    platform::{look_up, register, EncodedMessage, IoHub, IoMultiplexing, Remote},
    util::EndpointID,
    version::Version,
    Error, JoinError, Message, MessageBox, Options, RecvError, SendError,
};

/// Join a message bus.
///
/// The join timeout and every later receive deadline read the clock of `ctx`.
///
/// # Errors
///
/// Returns a [`JoinError`] when the endpoint cannot join the bus.
// The public API takes the context by value. Every caller passes an owned context.
#[allow(clippy::needless_pass_by_value)]
pub fn join<T: MessageBox, R: MessageBox>(
    options: Options,
    timeout: Option<Duration>,
    ctx: Ctx,
) -> Result<(EndpointSender<T>, EndpointReceiver<R>), JoinError> {
    let joined = Rule::join(options, 0, Arc::new(IoMultiplexing::new()), timeout, &ctx)?;
    tracing::debug!(endpoint_id = ?joined.endpoint_id(), "ipc endpoint joined the bus");
    let rule = Arc::new(RwLock::new(joined));

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
    /// Send a message to the bus.
    ///
    /// # Errors
    ///
    /// Returns a [`SendError`] when the endpoint must rejoin the bus and the join fails.
    ///
    /// # Panics
    ///
    /// Panics when the message carries more than 65535 memory regions.
    /// Panics when an internal lock is poisoned.
    pub fn send(&self, mut msg: Message<T>) -> Result<(), SendError> {
        msg.selector.memory_region_count = u16::try_from(msg.memory_regions.len())
            .expect("a message carries at most 65535 memory regions");
        let mut msg = msg.into_encoded();

        loop {
            let rule = self.rule.read().unwrap();
            match &*rule {
                Rule::Client {
                    remote, epoch, ctx, ..
                } => {
                    let ctx = ctx.clone();
                    match msg.send(remote) {
                        Err(Error::Disconnect) => {
                            let epoch = *epoch;
                            drop(rule);

                            let mut rule = self.rule.write().unwrap();
                            rejoin(&mut rule, epoch, None, &ctx)?;
                        }
                        Err(_) => unreachable!(),
                        Ok(()) => break Ok(()),
                    }
                }
                Rule::Server { bus_sender, im, .. } => {
                    bus_sender.lock().unwrap().send(msg).unwrap();
                    im.wake();
                    break Ok(());
                }
            }
        }
    }
}

/// Join the bus again after a disconnect.
///
/// Does nothing when another thread already rejoined (the `epoch` differs) or
/// when the rule is a server.
fn rejoin(
    rule: &mut Rule,
    epoch: u32,
    timeout: Option<Duration>,
    ctx: &Ctx,
) -> Result<(), JoinError> {
    let Rule::Client {
        options,
        io_hub,
        reader_closed,
        im,
        epoch: current_epoch,
        ..
    } = rule
    else {
        return Ok(());
    };
    if epoch != *current_epoch {
        return Ok(());
    }
    let reader_was_closed = *reader_closed;
    drop(io_hub.take());
    let joined = Rule::join(
        options.clone(),
        epoch.overflowing_add(1).0,
        im.clone(),
        timeout,
        ctx,
    )?;
    *rule = joined;
    if reader_was_closed {
        rule.reader_close();
    }
    Ok(())
}

/// Decode a received message.
///
/// Returns `Ok(None)` when the type uuid is not known to the receiver, so the caller skips it.
fn decode_received<R: MessageBox>(
    encoded_msg: EncodedMessage,
) -> Result<Option<Message<R>>, RecvError> {
    match R::decode(encoded_msg.selector.uuid, encoded_msg.payload_data) {
        Ok(payload) => {
            let mut msg = Message::new(encoded_msg.selector, payload);
            msg.objects = encoded_msg.objects;
            msg.memory_regions = encoded_msg.memory_regions;
            Ok(Some(msg))
        }
        Err(Error::TypeUuidNotFound) => Ok(None),
        Err(Error::Decode(err)) => Err(RecvError::Decode(err)),
        Err(_) => unreachable!(),
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
    /// Receive the next message for this endpoint.
    ///
    /// # Errors
    ///
    /// Returns [`RecvError::Timeout`] when no message arrives before `timeout`.
    /// Returns [`RecvError::Decode`] when the message payload is not valid.
    /// Returns another [`RecvError`] when the endpoint must rejoin the bus and the join fails.
    ///
    /// # Panics
    ///
    /// Panics when the reader is closed or when an internal lock is poisoned.
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
                    ctx,
                    ..
                } => {
                    let ctx = ctx.clone();
                    if !*reader_closed && io_hub.is_none() {
                        let epoch_val = *epoch;
                        drop(rule);

                        let mut rule = self.rule.write().unwrap();
                        rejoin(&mut rule, epoch_val, timeout, &ctx)?;
                        continue;
                    }

                    let mut io_hub_guard = io_hub.as_ref().expect("reader closed").lock().unwrap();

                    match io_hub_guard.recv(timeout, Some(remote), &ctx) {
                        Ok(encoded_msg) => {
                            if encoded_msg.selector.label_op.validate(&options.label) {
                                if let Some(msg) = decode_received(encoded_msg)? {
                                    break Ok(msg);
                                }
                            }
                        }
                        Err(Error::Disconnect) => {
                            let epoch_val = *epoch;
                            drop(io_hub_guard);
                            drop(rule);

                            let mut rule = self.rule.write().unwrap();
                            rejoin(&mut rule, epoch_val, timeout, &ctx)?;
                        }
                        Err(Error::Timeout) => {
                            break Err(RecvError::Timeout);
                        }
                        Err(_) => unreachable!(),
                    }
                }
                Rule::Server { receiver, .. } => {
                    let receiver = receiver.as_ref().expect("reader closed").lock().unwrap();
                    let encoded_msg = match timeout {
                        Some(timeout) => match receiver.recv_timeout(timeout) {
                            Ok(encoded_msg) => encoded_msg,
                            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                                break Err(RecvError::Timeout);
                            }
                            Err(_) => unreachable!(),
                        },
                        None => receiver.recv().unwrap(),
                    };
                    if let Some(msg) = decode_received(encoded_msg)? {
                        break Ok(msg);
                    }
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
        ctx: Ctx,
    },
    Server {
        endpoint_id: EndpointID,
        bus_sender: std::sync::Mutex<std::sync::mpsc::Sender<EncodedMessage>>,
        receiver: Option<std::sync::Mutex<std::sync::mpsc::Receiver<EncodedMessage>>>,
        im: Arc<IoMultiplexing>,
    },
}

impl Rule {
    /// The endpoint identity this rule joined the bus under.
    ///
    /// Carried on both variants so a disconnect or routing decision can name the
    /// endpoint; surfaced here so it is observable rather than dead.
    pub(crate) fn endpoint_id(&self) -> EndpointID {
        match self {
            Rule::Client { endpoint_id, .. } | Rule::Server { endpoint_id, .. } => *endpoint_id,
        }
    }

    #[allow(clippy::too_many_lines)] // One retry loop over every look-up and register outcome.
    fn join(
        options: Options,
        epoch: u32,
        im: Arc<IoMultiplexing>,
        timeout: Option<Duration>,
        ctx: &Ctx,
    ) -> Result<Self, JoinError> {
        let end = timeout.map(|t| ctx.clock().now() + t);

        macro_rules! wait {
            () => {
                let mut w = Duration::from_secs(2);
                if let Some(end) = end {
                    let remain = end.saturating_duration_since(ctx.clock().now());
                    if remain.is_zero() {
                        return Err(JoinError::Timeout);
                    }
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
                ctx,
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
                        ctx: ctx.clone(),
                    };
                }
                Err(Error::IdentifierNotInUse) => {
                    if !options.controller_affinity {
                        wait!();
                        continue;
                    }

                    let r = register(&options.identifier, im.clone());
                    match r {
                        Ok((io_hub, bus_sender, eid)) => {
                            let (tx, rx) = std::sync::mpsc::channel();
                            let im2 = io_hub.io_multiplexing();

                            let ctrl = super::platform::BusController::new(
                                eid,
                                options.label,
                                options.token,
                                tx,
                                io_hub,
                                ctx.clone(),
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
                            if perm_denied > 5 {
                                return Err(JoinError::PermissionDenied);
                            }
                            wait!();
                        }
                        Err(e) => {
                            tracing::error!("register: {:?}", e);
                            wait!();
                        }
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
                    if perm_denied > 5 {
                        return Err(JoinError::PermissionDenied);
                    }
                    wait!();
                }
                Err(Error::Timeout) => {
                    timeout_count += 1;
                    if timeout_count > 5 {
                        return Err(JoinError::VersionMismatch(Version::new()));
                    }
                    wait!();
                }
                Err(e) => {
                    tracing::error!("look_up: {:?}", e);
                    wait!();
                }
            }
        };

        Ok(rule)
    }
}

impl Rule {
    fn reader_close(&mut self) {
        match self {
            Rule::Client {
                io_hub,
                reader_closed,
                ..
            } => {
                let _ = io_hub.take();
                *reader_closed = true;
            }
            Rule::Server { receiver, .. } => {
                let _ = receiver.take();
            }
        }
    }
}
