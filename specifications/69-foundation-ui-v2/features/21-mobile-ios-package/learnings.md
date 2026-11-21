# Feature 21 learnings

- Swift Package Manager resolves the renderer from the repository root.
  Explicit target paths keep the Swift code and fixtures under `mobile/ios`.
- The todo actions and renderer use one linked Rust archive in the simulator package.
  This keeps a second copy of the native core out of the examples.
- The renderer reads presentation properties from the existing text value format.
  Fixtures without presentation roles still use the ordinary controls.
- A sheet and its underlying screen can have buttons with the same label.
  The interface tests select each Add button inside its own navigation bar.
