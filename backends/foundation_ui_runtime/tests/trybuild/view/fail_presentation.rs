use foundation_ui_runtime::html;

fn main() {
    let _ = html! { <Button presentation="navigation-title"/> };
    let _ = html! { <Button presentation="toolbar-action"/> };
    let _ = html! { <Button presentation="segmented-control"/> };
    let _ = html! { <Button presentation="inset-list"/> };
    let _ = html! { <Button presentation="checklist-row"/> };
    let _ = html! { <Button presentation="checkmark-action"/> };
    let _ = html! { <Button presentation="destructive-action"/> };
    let _ = html! { <Button presentation="list-footer"/> };
    let _ = html! { <Button presentation="confirmation-dialog"/> };
    let _ = html! { <Button presentation="sheet-title"/> };
    let _ = html! { <Button presentation="submit-action"/> };
    let _ = html! { <Button presentation="cancel-action"/> };
    let _ = html! { <Overlay presentation="form-sheet"/> };
}
