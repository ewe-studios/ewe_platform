# Feature 32. Target views and elements by name

The framework placement stages and count migration are complete. The Todo agent owns the remaining target migration under issue #714.

## Glossary

A view is a content area that a host registers with a renderer.
An identifier is a name that an author assigns with `id`.
A target selects a view or an element for a rendering instruction.
A frame carries instructions from a producer to a renderer.

## Framework contract

Every application can use the same placement markup on web, iOS, and Android.
Put target resolution in the framework. The todo example consumes the public contract.

| Author intent | Target | Resolution |
|---|---|---|
| Render an initial page | Omitted | Fill the receiving view. |
| Append content to a view | Omitted | Append to the receiving view. Preserve existing children. |
| Select a registered view | `view#view-1` | Find the named view in this host. |
| Select an element | `#open-count` | Find the name in the receiving view. |
| Check the element type too | `sheet#editor` | Match the name and its declared sheet type. |

A call or subscription carries its receiving view. Independent output can use the only registered view.
If several views qualify, require an explicit view. Never guess from focus or registration order.
Scope element lookup to that view. All shared element kinds support identifiers, including custom registered kinds.
A qualified target checks the shared tag type. The platform widget name cannot change the result.
Provide a common tag-name mapping. Accept `sheet` for the shared bottom sheet and document its canonical spelling.
Reserve `view` for registered view roots.

The author adds an identifier when another operation needs to address an element.
Keep identifiers unique within one view. Different views can use the same element name.
View names are unique within a host.
Application code must never assign numeric anchors or send updates to hard-coded node handles.
Use the receiving view or a named target for application updates.
The framework may allocate numeric handles internally for node operations and event dispatch.
Todo record identifiers identify records. They do not address rendering operations.

## Example

```rust
// Initial content for the receiving view.
let footer = html! { <Text id="open-count">0 open todos</Text> };

// A later publication uses the same declaration on each platform.
html_swap! {
    "#open-count", ctx, ctx.receiver(),
    <Text id="open-count">{count_label}</Text>
}
```

The replacement keeps the name so later publications can find it.

```mermaid
flowchart LR
    P[Publisher] --> C[One live channel]
    C --> W[Browser view]
    C --> I[iOS view]
    C --> A[Android view]
```

Each subscriber resolves the same named target inside its own view.

## Stage 1. Define the public syntax and wire contract

- Support names in shared markup, initial rendering, and all targeted placement macros.
- Add a target-free append form. Keep append and full-view replacement distinct.
- Use the existing wire identifier selector for bare names. Encode a type check and view selection explicitly when required.
- Specify name parsing, escaping, and malformed-input errors once. Use shared fixtures in every renderer.
- Preserve existing wire field numbers. Regenerate affected bindings and document capability requirements.
- Keep browser-specific selectors available through their existing path. Portable selectors must resolve identically on each platform.

## Stage 2. Implement placement in each framework renderer

- Register view roots at host setup. Bind action replies and stream deliveries to the receiving view.
- Index element names when mounting. Update the index on replacement, removal, and view disposal.
- Apply this contract to append, swap, remove, insert before, and insert after.
- For relative insertion, resolve the named child under the selected parent.
- Validate names and targets before changing the displayed tree. Permit a replacement to reuse names from the outgoing subtree.
- Report a missing target, duplicate name, wrong type, or ambiguous view. Preserve the affected view on failure.
- Apply the same rules to direct rendering, action replies, and streamed updates.

## Stage 3. Use a single shared publication

Publish shared instructions through one channel and one endpoint. All three renderers can apply the same portable frame.
Keep each subscriber’s transport, cursor, and cancellation independent. Preserve replay after reconnect.
Migrate the todo count to one named element and one publication on `/live`.
Remove the example’s separate native count route and numeric count target.
Keep shared selection and delivery logic in the framework. Example code supplies only the view and its business data.

## Stage 4. Use one endpoint for each screen

| Route | Frame content | Callers |
|---|---|---|
| `GET /initial` | Full todo view with a dialog area and a live count subscription | Browser, iOS, and Android todo apps |
| `GET /open-count` | Open count with a Refresh button bound to `refresh_count` | Browser, iOS, and Android starter clients |

Build each screen once with shared controls and named targets. Both handlers return a frame.
Use the same count subscription from the full view on each platform.
Point every starter at the count screen. Declare its refresh action on the browser.
Remove `/native-initial` and `/native-starter` from route registration, callers, tests, templates, and current setup guides.
Keep action submission and live subscription endpoints available.
Load both routes on all three platforms. Submit Refresh and verify that the displayed count matches the server.
Check that an update after initial load reaches the correct count target on each platform.

## Acceptance checks

1. Run common placement fixtures on web, Swift, and Kotlin. Check names on every shared element kind.
2. Verify default append, initial replacement, named views, qualified targets, and every relative placement.
3. Mount two views with identical child names. Deliver a reply to each. Check that its sibling stays unchanged.
4. Reject duplicate names, wrong types, missing children, and ambiguous destinations before mutation.
5. Replace a named subtree twice. Remove it, reuse its name, and verify cleanup when its view closes.
6. Deliver one captured count frame unchanged to all three renderers. Verify the displayed count on each platform.
7. Connect web and native subscribers to the same running endpoint. Publish once and verify independent resume.
8. Update the framework and mobile package README files. Show default placement, explicit view selection, and a typed element target.

Use the existing [mobile designs](../23-native-design-and-themes/feature.md) for the todo regression check.

## Remaining Todo targets

Issue #714 tracks the application targets added during the synchronization repair.
The following revision supersedes the earlier full-page count publication. The separate count screen still demonstrates server rendering.

### Action ownership

The server owns todo records. Each client keeps a local copy for display and search.
Each action has one owner and one purpose.

| Intent | Owner | Result |
|---|---|---|
| Open, cancel, or dismiss a dialog | Host | Update the named dialog area on this client. |
| Submit a title or confirm deletion | Server | Validate the request and change the server store. |
| Complete or reopen a todo | Server | Change the record and publish the resulting status. |
| Search or filter | Host | Select matching cached records and update this client's list. |
| Open or close Mathalley | Host | Start or release the game view described in Feature 36. |

Build the initial page once. Give each addressed element a name in the shared markup.
Use one dialog builder. Its inputs select the dialog kind and the affected todo record.
Use declarative controls such as `Either` to select visible content. Keep each action binding in one place.
The closed, add, and delete states share the named dialog area. Only deletion carries a target record.
Submitting a title records a pending row before sending the server action `add_todo`. Bind deletion confirmation to `confirm_delete`. Bind cancellation to `close_dialog`.
Remove the duplicate device mutation handlers and their registrations.
Remove the `local` argument and the `OnDevice` checks from page, row, dialog, filter, and mutation code.
Update callers of the standalone mobile constructor. Its host actions use the cache. Record changes require a server connection.
Declare any platform-specific action scope at its registration and binding boundary.
Use the same shared page and controls on each supported host.

### Local cache and delivery

The initial page request returns the shared page, its record snapshot, and the matching stream position.
The host seeds its local store from that snapshot and retains outstanding submissions during recovery.
One local rendering action owns the rows and the named open-count footer on the full Todo page.
Keep the separate count screen as an example of a server action that renders a view.

A server write returns a typed record change with its committed stream position.
The server publishes that same change to subscribers.
Both paths enter one local action that applies changes in stream order.
Ignore a duplicate position. Request missing changes before advancing across a gap.
Recover from an expired stream with a fresh snapshot and position.
Match pending additions by record identifier. Keep local delivery status separate from confirmed records.
Show pending and failed submissions from local state. Derive the open count from confirmed records in that same store.

After applying a change, invoke the shared local rendering action for the affected named regions.
Keep search input, focus, scroll position, and open dialogs during record updates.
While search is active, keep its displayed rows fixed until the user changes or clears the query.
The write reply and live publication carry the committed record change.
A receipt alone leaves an addition pending. The store confirms it after applying the committed change in order.
Render the changed row and footer from the same resulting store revision.
Test reply-first, publication-first, duplicate delivery, and a gap before the committed change.
During search, verify that the footer updates while the displayed search results remain fixed.

```mermaid
sequenceDiagram
    participant H as Host
    participant L as Local store
    participant S as Todo server
    H->>L: Save pending addition
    L-->>H: Render pending row
    H->>S: Submit addition with stable identifier
    S-->>H: Return committed change and position
    S-->>H: Publish same change through live stream
    H->>L: Apply each committed position once
    L-->>H: Render current local state
```

Either delivery can arrive first. The local store reconciles both before the host renders confirmed records.

### Acceptance checks for this revision

1. Inspect the initial page and dialog bindings on Chrome, iPhone, and Android.
2. Verify that each mutation invokes exactly one server action. Verify that opening and cancelling send no request.
3. Freeze search results during remote changes. Clear the query and verify all changes against the cache.
4. Replay a duplicate update. Expire the replay history. Verify recovery without duplicate rows or lost local input.
5. Deliver a cache instruction without any rendering operation. Verify the receiving client's cache independently of the count screen.
6. Verify named targets and displayed rows on all three renderers. Inspect the outgoing frames for numeric application targets.
7. Verify that platform-scoped actions respect their declared scope through framework dispatch.
8. Use Feature 36 to verify game startup, continued todo delivery, and resource release when the game closes.

### Optimistic additions

Optimistic display shows a pending record before the server confirms it.
Generate one universally unique identifier (UUID) for each new todo through the injected host service.
Store the identifier and original submission locally before dispatch. Close the add dialog locally.
Show the pending row immediately with a faded appearance and a gentle pulse.
Show a pending label. Respect reduced motion settings.
Retry sends the same identifier and original submission.
The server records acceptance atomically with the new todo and recognizes repeated submissions.
A conflicting creation payload for the same identifier fails. A late retry after deletion must never recreate the record.
Use the live record or a recovery snapshot to reconcile the pending row by identifier.
An acknowledgement can precede or follow that record. A late timeout cannot revert a confirmed row.
Show Try again for an uncertain or temporary failure. Preserve the title for correction after a validation error.
Keep pending submissions across an app restart. Test two distinct additions with identical titles.
Keep the open count authoritative across pending, accepted, and replayed additions.

### Optimistic deletion

After the user confirms deletion, save the pending deletion locally and close the confirmation dialog.
Keep the row visible with a “Deleting…” label. Disable further changes to that row.
Send the delete request with the same todo identifier on every attempt.
The server makes repeated requests for the same deletion safe.
A confirmed removal from either delivery resolves the pending deletion, including a removal from another client.
Remove the row once. Update the confirmed open count once if the removed todo was open.
A temporary failure keeps the row and shows **Try again**. A late failure cannot restore a confirmed deletion.
On restart, reconcile pending deletions against the recovered records and stream position.
During search, show feedback for the user's own deletion. Keep unrelated search results fixed.
Verify delayed replies, repeated taps, retry after restart, and concurrent deletion on two clients.
This optimistic behavior applies to Add and Delete. Completion and reopening continue to use confirmed updates.

### Search during delivery

The host owns the visible list. Remote changes update the cache while a search is active.
Preserve the displayed search results, including when an incoming title matches the query.
Recompute from the latest cache when the user changes or clears the query. Preserve the selected filter.
Validate actions on a frozen row against its current record. Handle remote deletion without affecting another record.

### Separate one-minute block demonstration

A host action requests a one-minute block for the initiating client. Other clients retain their own interaction state.
The server returns a block identifier and expiry. The client shows a modal countdown and blocks application input.
The server sends a release for that block. The host removes the matching overlay.
Keep live delivery active during the block. Reconcile the current block on reconnect, resume, or restart.
Ignore a stale release for an older block. Test a lost request and a lost release.
Verify pointer, keyboard, back navigation, and assistive input on all three hosts.
Use an injected clock for timing tests. Capture pending, retry, and blocked screens on each host during implementation.
