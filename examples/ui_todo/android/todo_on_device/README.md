# Todo On Device

The app starts a local Rust core. The core keeps the store for the screen session and returns frames for every action.
The app requests no Internet permission and starts without a server.

[Build and install both apps](../README.md). Then start the app on the selected emulator.

```sh
adb shell am start -n ewe.todo.local/.MainActivity
```

Tap **Add**, enter a title and tap **Add** in the sheet. The open count increases.
Select **Complete**, **Done**, and **Reopen** to check filter and count changes.
Select **Open**, **Delete**, and **Confirm** to remove the task.
