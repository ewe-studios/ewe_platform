#if os(iOS)
  import SwiftUI

  extension NodeRecord {
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
          $0.kind == .stack && records($0).contains(where: { $0.kind == .navigationTitle })
        }) else { continue }
        let siblings = containers.filter { $0 !== container }
        let content = siblings.flatMap { sibling -> [NodeRecord] in
          let children = records(sibling)
          if sibling.kind == .stack, children.count == 1, let child = children.first,
            [NodeKind.listFooter, .modal, .confirmationDialog].contains(child.kind)
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
      let title = children.first { $0.kind == .navigationTitle }
      let count = children.first { $0.kind == .listFooter }
      let hasList = children.contains { $0.kind == .insetList }
      let controls = children.filter {
        $0.kind != .navigationTitle && $0.kind != .toolbarButton
          && (!hasList || $0.kind != .listFooter)
      }
      return NavigationStack {
        EweColumn(nodes: controls, alignment: .center, spacing: 0) { child in
          if child.kind == .insetList {
            EweInsetList(screen: self, node: child, footer: count)
          } else {
            RecordView(screen: self, n: child)
          }
        }
        .navigationTitle(title?.text ?? "")
        .toolbar {
          ToolbarItemGroup(placement: .primaryAction) {
            ForEach(children.filter { $0.kind == .toolbarButton }, id: \.identity) { action in
              EweToolbarButton(node: action) { sendTap(action) }
                .modifier(EweNodeStyle(theme: theme, node: action))
            }
          }
        }
      }
    }

    func presentation(_ node: NodeRecord) -> AnyView? {
      let children = records(node)
      if node.kind == .stack, children.contains(where: { $0.kind == .navigationTitle }) {
        return AnyView(navigation(node, supplemental: []))
      }
      switch node.kind {
      case .segmentedControl:
        return AnyView(EweSegmentedControl(screen: self, node: node, options: children))
      case .insetList:
        return AnyView(EweInsetList(screen: self, node: node, footer: nil))
      case .checklistRow:
        return AnyView(EweChecklistRow(screen: self, node: node, children: children))
      case .listFooter:
        return AnyView(EweListFooter(node: node)
          .frame(maxWidth: .infinity, alignment: .leading)
          .padding(.horizontal)
          .padding(.bottom, 12))
      case .confirmationDialog:
        return AnyView(EweConfirmationDialog(screen: self, node: node).id(node.identity))
      default:
        return nil
      }
    }

    fileprivate func actionButton(_ node: NodeRecord, role: ButtonRole? = nil) -> some View {
      Button(node.text ?? "", role: role) { sendTap(node) }
        .modifier(EweNodeStyle(theme: theme, node: node))
        .disabled(!node.isEnabled)
        .accessibilityLabel(node.label)
    }

    fileprivate func sendTap(_ node: NodeRecord) {
      bridge.send(.tap, node: node.id, value: node.value)
    }

    fileprivate func presentationContent(_ node: NodeRecord) -> [NodeRecord] {
      records(node).flatMap { $0.kind == .stack ? records($0) : [$0] }
    }
  }

  @MainActor private struct EweInsetList: View {
    let screen: EweScreen
    let node: NodeRecord
    let footer: NodeRecord?
    var body: some View {
      List {
        Section {
          ForEach(screen.records(node), id: \.identity) { child in
            EweScreen.RecordView(screen: screen, n: child)
          }
        } footer: {
          if let footer {
            EweListFooter(node: footer)
              .modifier(EweNodeStyle(theme: screen.theme, node: footer))
          }
        }
      }
      .listStyle(.insetGrouped)
    }
  }

  @MainActor private struct EweSegmentedControl: View {
    let screen: EweScreen
    let node: NodeRecord
    let options: [NodeRecord]
    var body: some View {
      Picker(node.label, selection: Binding(
        get: { options.first(where: \.isChecked)?.id },
        set: { id in
          if let action = options.first(where: { $0.id == id }), action.isEnabled {
            screen.sendTap(action)
          }
        }
      )) {
        ForEach(options, id: \.identity) { option in
          Text(option.text ?? "").accessibilityLabel(option.label)
            .modifier(EweNodeStyle(theme: screen.theme, node: option))
            .tag(Optional(option.id))
            .disabled(!option.isEnabled)
        }
      }
      .pickerStyle(.segmented)
      .labelsHidden()
      .disabled(!node.isEnabled)
      .padding(.horizontal)
    }
  }

  @MainActor private struct EweChecklistRow: View {
    let screen: EweScreen
    let node: NodeRecord
    let children: [NodeRecord]
    var body: some View {
      let status = children.first { $0.kind == .checkmarkButton }
      let delete = children.first { $0.kind == .destructiveButton }
      return HStack(alignment: .firstTextBaseline, spacing: 12) {
        if let status {
          EweCheckmarkButton(node: status, checked: node.isChecked) { screen.sendTap(status) }
          .modifier(EweNodeStyle(theme: screen.theme, node: status))
        }
        ForEach(children.filter { $0.kind != .checkmarkButton && $0.kind != .destructiveButton },
          id: \.identity) { child in
          EweScreen.RecordView(screen: screen, n: child)
            .strikethrough(node.isChecked && child.kind == .text)
        }
      }
      .disabled(!node.isEnabled)
      .swipeActions(edge: .trailing, allowsFullSwipe: false) {
        if let delete {
          screen.actionButton(delete).tint(.red)
        }
      }
      .accessibilityActions {
        if let delete { screen.actionButton(delete) }
      }
    }
  }

  @MainActor private struct EweConfirmationDialog: View {
    let screen: EweScreen
    let node: NodeRecord
    @State private var presented = false
    var body: some View {
      let controls = screen.presentationContent(node)
      let title = controls.first { $0.kind == .sheetTitle }?.text ?? ""
      Color.clear.frame(width: 0, height: 0)
        .onAppear { presented = true }
        .alert(title, isPresented: $presented) {
          ForEach(controls.filter { $0.kind == .destructiveButton || $0.kind == .submitButton || $0.kind == .button }, id: \.identity) { action in
            screen.actionButton(action, role: action.kind == .destructiveButton ? .destructive : nil)
          }
          if let cancel = controls.first(where: { $0.kind == .cancelButton }) {
            screen.actionButton(cancel, role: .cancel)
          }
        } message: {
          ForEach(controls.filter { $0.kind == .text || $0.kind == .listFooter }, id: \.identity) { child in
            Text(child.text ?? "")
          }
        }
    }
  }
#endif
