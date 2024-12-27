/// TaskStatus represents the current state of a computation to be
/// completed and deliverd from the iterator.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TaskStatus<P, D> {
    /// Pending is a state indicative of the status
    /// of still awaiting the readiness of some operations
    /// this can be the an underlying process waiting for
    /// some timeout to expire or a response to be received
    /// over IO or the network.
    ///
    /// Generally you send this to indicate the task as still
    /// being in a state of processing.
    Pending(P),

    /// Init represents a middle point state where the process
    /// may not immediately move into a ready state e.g reconnect
    /// to some remote endpoint or to trigger some actual underlying
    /// processes that get us into a ready state with
    /// the relevant result.
    Init,

    /// Ready is the final state where we consider the task
    /// has finished/ended with relevant result.
    Ready(D),
}

/// AsTaskIterator represents a type for an iterator with
/// the underlying output of the iterator to be `TaskStatus`
/// and it's relevant semantics.
pub trait AsTaskIterator<P, D>: Iterator<Item = TaskStatus<P, D>> {}
