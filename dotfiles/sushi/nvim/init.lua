local my_config = require("tarberd")
local keymaps = require("tarberd.keymaps")

my_config.setup()
keymaps.global_lsp()

vim.lsp.inlay_hint.enable(true)
vim.diagnostic.config({
  virtual_text = false,
  severity_sort = true,
  float = {
    scope = "cursor",
  },
})

vim.api.nvim_create_autocmd("LspAttach", {
  group = vim.api.nvim_create_augroup("tarberd_lsp_attach", { clear = true }),
  callback = function(event)
    -- This calls your keymap function and passes the current buffer
    require("tarberd.keymaps").create_buffer_lsp_keymaps(nil, event.buf)
  end,
})

-- Get LSP capabilities from blink.cmp
local capabilities = require("blink.cmp").get_lsp_capabilities()

-- Note: If you ever want to apply these capabilities globally to ALL servers you add in the future,
-- you can uncomment the following line (supported in Neovim 0.11+):
-- vim.lsp.config("*", { capabilities = capabilities })

vim.lsp.config("nixd", {
  capabilities = capabilities,
  settings = {
    nixd = {
      nixpkgs = {
        expr = 'import <nixpkgs> { }',
      },
      formatting = {
        command = { "nixfmt" },
      },
      options = {
        nixos = {
          expr = '(builtins.getFlake (toString ./.)).nixosConfigurations.gandalf.options',
        },
        sauron = {
          expr = '(builtins.getFlake (toString ./.)).nixosConfigurations.sauron.options',
        },
        nixvirt = {
          expr = '(builtins.getFlake (toString ./.)).inputs.nixvirt', -- or its exported options/modules
        },
      },
    },
  },
})
vim.lsp.enable("nixd")
