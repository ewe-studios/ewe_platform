You can refresh a file to its latest content from disk manually using a command in Neovim, or automatically by configuring the autoread setting.

### Manual Refresh

To manually reload the current buffer from the file on the disk:

```vim
:e
```

In Normal mode, type the following command and press Enter:

```vim
:e
```

If you have made unsaved changes in Neovim that you want to discard, add a ! to the command:

```vim
:e!
```

This will force a reload and all unsaved changes will be lost.

### Automatic Refresh

To have Neovim automatically detect and reload files that have been changed externally, you can set the autoread option in your configuration file (init.vim or init.lua).

- Vimscript (init.vm)

```
set autoread
```

- Lua (init.lua)

```lua
vim.opt.autoread = true

```

Setting autoread enables a basic auto-reload feature. Neovim will check for changes on disk when you switch between buffers or windows.
For a more immediate and robust automatic reload, especially when switching focus back to Neovim from another application, you can use an autocmd to trigger a checktime command:

- Vimscript (init.vim)

```vim

autocmd FocusGained,BufEnter,CursorHold,CursorHoldI * if mode() != 'c' | checktime | endif

```

- Lua (init.lua)

```lua
vim.api.nvim_create_autocmd({"FocusGained", "BufEnter", "CursorHold", "CursorHoldI"}, {
  group = vim.api.nvim_create_augroup("CheckFileChanges", { clear = true }),
  callback = function()
    if vim.api.nvim_get_mode().mode ~= 'c' then
      vim.cmd("checktime")
    end
  end
})

```

This configuration uses events like FocusGained (when you focus the Neovim window) to run checktime, which prompts Neovim to update the buffer if the file on disk is newer.
