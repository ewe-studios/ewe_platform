/// Notifable is a type that can be notified when the AsyncIterator
/// is ready.
pub trait Notifiable<T> {
    fn notify(&self, t: T);
}

/// [`CanNotifyOfSelf`] defines a type whose underlying behave can deliver
/// notification of it's self to others.
///
/// It exposes a [`CanNotify::register`] method that allows you
/// register for notification with it where it notifies you of it's
/// state when received.
pub trait CanNotify<V> {
    fn register(&self, t: impl Notifiable<V>);
}

/// CanNotifyOfSelf defines a type whose underlying behave can deliver
/// notification of it's self to others.
///
/// It exposes a [`CanNotify::register`] method that allows you
/// register for notification with it where it notifies you of it's
/// state when received.
pub trait CanNotifyOfSelf: CanNotify<Self>
where
    Self: Sized,
{
}
