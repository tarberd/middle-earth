local my_config = require("tarberd")
local keymaps = require("tarberd.keymaps")

my_config.setup()
keymaps.global_lsp()

vim.api.nvim_create_autocmd("LspAttach", {
  callback = function(args)
    local client = vim.lsp.get_client_by_id(args.data.client_id)
    if client:supports_method("textDocument/inlayHint") then
      vim.lsp.inlay_hint.enable(true, { bufnr = args.buf })
      end
  end,
})

-- rust-analyzer answers the first inlay hint request before it finishes
-- indexing, and its refreshes during indexing get cancelled, so hints stay
-- empty until the first edit. Re-request them whenever server work finishes.
vim.api.nvim_create_autocmd("LspProgress", {
  pattern = "end",
  callback = function(args)
    for buf in pairs(vim.lsp.get_client_by_id(args.data.client_id).attached_buffers) do
      if vim.lsp.inlay_hint.is_enabled({ bufnr = buf }) then
        vim.lsp.inlay_hint.enable(true, { bufnr = buf })
      end
    end
  end,
})

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

vim.lsp.config("rust_analyzer", {
  capabilities = capabilities,
  settings = {
    ["rust-analyzer"] = {
      check = {
        command = "clippy",
      },
    },
  },
})
vim.lsp.enable("rust_analyzer")
