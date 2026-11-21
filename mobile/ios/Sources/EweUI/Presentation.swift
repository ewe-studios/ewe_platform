#if os(iOS)
  import SwiftUI

  extension NodeRecord {
    var role: String? { props[.presentation]?.description }
    var isEnabled: Bool { props[.enabled] != .bool(false) }
    var label: String { props[.a11yLabel]?.description ?? text ?? "" }
    var value: String { props[.value]?.description ?? "" }
    var isChecked: Bool { props[.checked] == .bool(true) }
  }

  @MainActor extension EweScreen {
    func rootNavigation(_ root: NodeRecord) -> AnyView? {
      let containers = records(root)
      for container in containers {
        let descendants = records(container)
        let candidates = descendants.count == 1 ? [container, descendants[0]] : [container]
        guard let page = candidates.first(where: {
          $0.kind == .stack && records($0).contains(where: { $0.role == "navigation-title" })
        }) else { continue }
        let siblings = containers.filter { $0 !== container }
        let content = siblings.flatMap { sibling -> [NodeRecord] in
          let children = records(sibling)
          if sibling.kind == .stack, children.count == 1, let child = children.first,
            ["list-footer", "form-sheet", "confirmation-dialog"].contains(child.role ?? "")
          {
            return [child]
          }
          return [sibling]
        }
        return AnyView(navigation(page, supplemental: content))
      }
      return nil
    }

    private func navigation(_ page: NodeRecord, supplemental: [NodeRecord]) -> some View {
      let children = records(page) + supplemental
      let title = children.first { $0.role == "navigation-title" }
      let count = children.first { $0.role == "list-footer" }
      let hasList = children.contains { $0.role == "inset-list" }
      return NavigationStack {
        VStack(spacing: 0) {
          let controls = children.filter {
            $0.role != "navigation-title" && $0.role != "toolbar-action"
              && (!hasList || $0.role != "list-footer")
          }
          ForEach(controls, id: \.identity) { child in
            if child.role == "inset-list" {
              insetList(child, footer: count)
            } else {
              RecordView(screen: self, n: child)
            }
          }
        }
        .navigationTitle(title?.text ?? "")
        .toolbar {
          ToolbarItemGroup(placement: .primaryAction) {
            ForEach(children.filter { $0.role == "toolbar-action" }, id: \.identity) { action in
              actionButton(action)
            }
          }
        }
      }
    }

    func presentation(_ node: NodeRecord) -> AnyView? {
      let children = records(node)
      if node.kind == .stack, children.contains(where: { $0.role == "navigation-title" }) {
        return AnyView(navigation(node, supplemental: []))
      }
      switch (node.kind, node.role) {
      case (.stack, "segmented-control"):
        return AnyView(segmented(node, options: children))
      case (.list, "inset-list"):
        return AnyView(insetList(node, footer: nil))
      case (.stack, "checklist-row"):
        return AnyView(checklistRow(node, children: children))
      case (.text, "list-footer"):
        return AnyView(Text(node.text ?? "")
          .font(.footnote)
          .foregroundStyle(.secondary)
          .accessibilityLabel(node.label)
          .frame(maxWidth: .infinity, alignment: .leading)
          .padding(.horizontal)
          .padding(.bottom, 12))
      case (.overlay, "form-sheet"):
        return AnyView(FormPresentation(screen: self, node: node).id(node.identity))
      case (.overlay, "confirmation-dialog"):
        return AnyView(ConfirmationPresentation(screen: self, node: node).id(node.identity))
      default:
        return nil
      }
    }

    private func insetList(_ node: NodeRecord, footer: NodeRecord?) -> some View {
      List {
        Section {
          ForEach(records(node), id: \.identity) { child in
            RecordView(screen: self, n: child)
          }
        } footer: {
          if let footer {
            Text(footer.text ?? "").accessibilityLabel(footer.label)
          }
        }
      }
      .listStyle(.insetGrouped)
    }

    private func segmented(_ node: NodeRecord, options: [NodeRecord]) -> some View {
      Picker(node.label, selection: Binding(
        get: { options.first(where: \.isChecked)?.id },
        set: { id in
          if let action = options.first(where: { $0.id == id }), action.isEnabled {
            sendTap(action)
          }
        }
      )) {
        ForEach(options, id: \.identity) { option in
          Text(option.text ?? "").accessibilityLabel(option.label)
            .tag(Optional(option.id))
            .disabled(!option.isEnabled)
        }
      }
      .pickerStyle(.segmented)
      .labelsHidden()
      .disabled(!node.isEnabled)
      .padding(.horizontal)
    }

    private func checklistRow(_ node: NodeRecord, children: [NodeRecord]) -> some View {
      let status = children.first { $0.role == "checkmark-action" }
      let delete = children.first { $0.role == "destructive-action" }
      return HStack(alignment: .firstTextBaseline, spacing: 12) {
        if let status {
          Button {
            sendTap(status)
          } label: {
            Image(systemName: node.isChecked ? "checkmark.circle.fill" : "circle")
              .font(.title3)
              .foregroundStyle(node.isChecked ? Color.secondary : Color.accentColor)
              .frame(minWidth: 44, minHeight: 44)
              .contentShape(Rectangle())
          }
          .buttonStyle(.borderless)
          .disabled(!status.isEnabled)
          .accessibilityLabel(status.label)
        }
        ForEach(children.filter { $0.role != "checkmark-action" && $0.role != "destructive-action" },
          id: \.identity) { child in
          RecordView(screen: self, n: child)
            .strikethrough(node.isChecked && child.kind == .text)
        }
      }
      .disabled(!node.isEnabled)
      .swipeActions(edge: .trailing, allowsFullSwipe: false) {
        if let delete {
          actionButton(delete).tint(.red)
        }
      }
      .accessibilityActions {
        if let delete { actionButton(delete) }
      }
    }

    fileprivate func actionButton(_ node: NodeRecord, role: ButtonRole? = nil) -> some View {
      Button(node.text ?? "", role: role) { sendTap(node) }
        .disabled(!node.isEnabled)
        .accessibilityLabel(node.label)
    }

    fileprivate func sendTap(_ node: NodeRecord) {
      bridge.send(.tap, node: node.id, value: node.value)
    }

    fileprivate func presentationContent(_ node: NodeRecord) -> [NodeRecord] {
      records(node).flatMap { $0.kind == .stack && $0.role == nil ? records($0) : [$0] }
    }
  }

  @MainActor private struct FormPresentation: View {
    let screen: EweScreen
    let node: NodeRecord
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize
    @State private var presented = false
    var body: some View {
      let controls = screen.presentationContent(node)
      let cancel = controls.first { $0.role == "cancel-action" }
      Color.clear.frame(width: 0, height: 0)
        .onAppear { presented = true }
        .sheet(isPresented: $presented, onDismiss: {
          if screen.store.record(node.id) === node, let cancel { screen.sendTap(cancel) }
        }) {
          sheet(controls)
            .presentationDetents(dynamicTypeSize.isAccessibilitySize ? [.large] : [.medium, .large])
            .presentationDragIndicator(.visible)
            .interactiveDismissDisabled(cancel == nil)
        }
    }
    @ViewBuilder private func sheet(_ controls: [NodeRecord]) -> some View {
      let inputs = controls.filter { $0.kind == .textInput }
      let titles = controls.filter { $0.role == "sheet-title" }
      let cancel = controls.filter { $0.role == "cancel-action" }
      let submit = controls.filter { $0.role == "submit-action" }
      if let input = inputs.first, inputs.count == 1, titles.count <= 1, cancel.count <= 1,
        submit.count <= 1, controls.count == inputs.count + titles.count + cancel.count + submit.count
      {
        FormContents(screen: screen, controls: controls, input: input)
      } else {
        NavigationStack {
          Form {
            ForEach(screen.records(node), id: \.identity) { control in
              EweScreen.RecordView(screen: screen, n: control)
            }
          }
        }
      }
    }

  }

  @MainActor private struct FormContents: View {
    let screen: EweScreen
    let controls: [NodeRecord]
    let input: NodeRecord
    @State private var title = ""

    var body: some View {
      NavigationStack {
        Form {
          TextField(input.props[.placeholder]?.description ?? "", text: $title)
            .accessibilityLabel(input.props[.a11yLabel]?.description ?? input.props[.placeholder]?.description ?? "")
            .disabled(!input.isEnabled)
            .onAppear { title = input.value }
            .onChange(of: title) { _, value in screen.bridge.send(.change, node: input.id, value: value) }
            .onSubmit(submit)
        }
        .navigationTitle(controls.first { $0.role == "sheet-title" }?.text ?? "")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
          ToolbarItem(placement: .cancellationAction) {
            if let cancel = controls.first(where: { $0.role == "cancel-action" }) {
              screen.actionButton(cancel, role: .cancel)
            }
          }
          ToolbarItem(placement: .confirmationAction) {
            if let add = controls.first(where: { $0.role == "submit-action" }) {
              Button(add.text ?? "") {
                if add.actions[.tap] != nil {
                  screen.bridge.send(.tap, node: add.id, value: title)
                } else {
                  submit()
                }
              }
              .disabled(!add.isEnabled || !input.isEnabled)
              .accessibilityLabel(add.label)
            }
          }
        }
      }
    }

    private func submit() { screen.bridge.send(.submit, node: input.id, value: title) }
  }

  @MainActor private struct ConfirmationPresentation: View {
    let screen: EweScreen
    let node: NodeRecord
    @State private var presented = false
    var body: some View {
      let controls = screen.presentationContent(node)
      let title = controls.first { $0.role == "sheet-title" }?.text ?? ""
      Color.clear.frame(width: 0, height: 0)
        .onAppear { presented = true }
        .alert(title, isPresented: $presented) {
          if let confirm = controls.first(where: { $0.role == "destructive-action" }) {
            screen.actionButton(confirm, role: .destructive)
          }
          if let cancel = controls.first(where: { $0.role == "cancel-action" }) {
            screen.actionButton(cancel, role: .cancel)
          }
        }
    }
  }
#endif
