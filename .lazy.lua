return {
	"neovim/nvim-lspconfig",
	opts = {
		servers = {
			["rust-analyzer"] = {
				settings = {
					["rust-analyzer"] = {
						cargo = {
							target = "thumbv6m-none-eabi",
						},
						check = {
							allTargets = false,
						},
					},
				},
			},
		},
	},
}
