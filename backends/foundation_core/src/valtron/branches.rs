use derive_more::Display;

/// [`CollectionState`] defines the possible state different
/// collect style operations can undergo, allowing us communicate
/// the different behaviour to be applied when we see a given
/// value or invocation.
#[derive(Display)]
pub enum CollectionState {
    /// Skip indicates we should skip the item/pass it on to the
    /// continuation task.
    Skip,
    /// Collect indicates this item should be collected.
    Collect,
    /// Close indicate no more collection should be done after this
    /// invocation, the bool within allows us indicate if the item
    /// seen during the close should be collected or ignored/skipped.
    Close(bool),
}

/// [`BranchPath`] defines the pathway of a giving execution
/// allowing a result to either go one path or another.
///
/// Shape:
///
/// let (branch_one, branch_two) = iterator.map_branch(|item| {
///     if condition1 {
///         return Branch::Left(item);
///     }
///     if condition2 {
///         return Branch::Right(item);
///     }
///     Branch::SKIP
/// });
///
/// Then people can use `branch_one` and `branch_two` wherever
/// they care and want.
///
/// This will allow us to support a `map_branch` combinative type
/// for [`TaskIterators`] and [`StreamIterators`] where
/// calling a function called [`map_branch`] can take a
/// function which when giving a [`TaskStatus`] or [`Stream`]
/// will return a `BranchPath` which will decide if it goes
/// to a left receiver or a right receiver.
///
/// This means [`map_branch`] will return two iterators:
///
/// 1. [`TaskIterator`] - when built from a [`TaskIterator`], this will
///    produce two task iterators that have been passed to the `execute`
///    method, producing the stream iterators necessary for these tasks.
///    The idea is that the two derived stream iterators represent the paths,
///    and hence you need not access the original you mapped from since its
///    result will go somewhere. Importantly, the main task iterator
///    itself has also been passed to `execute` with the [`unified::send()`] method
///    first before scheduling the branch iterators with [`unified::execute()`].
///
/// 2. [`StreamIterator`] - when built from an existing [`StreamIterator`], this will
///    produce two new stream iterators that each have their own [`ConcurrentQueue`].
///    The original stream iterator will use these to send their branches, and the main
///    stream iterator will be owned by both via a shared `Arc<Wrapper>` of some kind
///    that the other stream iterators will be able to call to trigger the next
///    stream state. This way, there does not need to be a central driver, just the
///    two streams driving the operation of the first when triggered. Since `map_branch`
///    always consumes `self`, this should be easy to do.
///
/// In both cases of [`TaskIterators`] and [`StreamIterators`], `self` is always owned and consumed.
/// In the case of [`TaskIterators`], after setup and wrapping, it is scheduled off by [`unified::send`].
/// and in the case of [`StreamIterators`] a wrapper owns self and is wrapped with a Arc and shared
/// with both the getting it to call some method like `tick()` which will call the predicate with
/// the result from the main iterator and deliver it to the correct branch.
#[derive(Display)]
pub enum BranchPath<L, R> {
    /// indicates no branch gets value and should just get a ignore/skip status.
    SKIP,

    /// Indicates the left hand of the map should get the value returned.
    Left(L),

    /// Indicates the right hand of the map should get the value reutrned.
    Right(R),
}
