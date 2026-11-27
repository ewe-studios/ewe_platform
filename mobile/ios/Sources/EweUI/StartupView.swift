import SwiftUI

/// Display progress while a host starts. A failed start exposes the retry action.
public struct EweStartupView: View {
  private let title: String
  private let issue: String?
  private let retryTitle: String
  private let retry: () -> Void

  public init(_ title: String, issue: String?, retryTitle: String = "Retry", retry: @escaping () -> Void) {
    self.title = title
    self.issue = issue
    self.retryTitle = retryTitle
    self.retry = retry
  }

  public var body: some View {
    VStack(spacing: 12) {
      if let issue {
        Text(issue).multilineTextAlignment(.center)
        Button(retryTitle, action: retry).buttonStyle(.bordered)
      } else {
        ProgressView(title)
      }
    }
    .padding()
    .frame(maxWidth: .infinity, maxHeight: .infinity)
  }
}
