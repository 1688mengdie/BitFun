# DeepSeek Harness (dsh) Adapter

This crate owns the static, runtime-free projection of DeepSeek Harness (`dsh`)
plugin package sources. It reads a BitFun-managed package whose
`bitfun.plugin.json` declares `adapter: "dsh_compatible"`, parses the package's
`package.json` `dsh` declaration, and projects the discovered bundle entries
(`dsh.bundle.patch` -> `cordis.patch.yml` rows) or profile bundle references
(`dsh.profile.bundles`) as projection-only plugin sources.

It does not execute Cordis plugins, install npm packages, or depend on a
user-local `dsh` CLI. Execution of dsh bundles belongs to future Plugin Host /
external-ACP work, not this adapter boundary.

## Boundary Rules

- Depend on stable contracts (`bitfun-runtime-ports`, `bitfun-product-domains`)
  and the `PluginRuntimeAdapter` boundary trait. Do not depend on `bitfun-core`,
  app crates, Tauri APIs, product UI, or concrete service managers.
- Keep the dsh `package.json` `dsh` field shape and `cordis.patch.yml` entry
  extraction inside this crate. Cross-crate outputs use typed
  `PluginSourceRef` / `PluginStatusSnapshot` / `PluginDiagnostic` DTOs; do not
  expose raw dsh YAML or JSON as product contracts.
- dsh bundles contribute Cordis services, not model-facing tool candidates, so
  `load_dsh_package_adapter` returns no provider dispatch targets. Unsupported
  or unparsable content must produce typed invalid projections and diagnostics,
  never silent success.
- New ecosystems are sibling adapters registered by Product Assembly
  (`bitfun-core/plugin_runtime`), not modes of this adapter.

## Verification

- `cargo test -p bitfun-dsh-adapter`
- `cargo test -p bitfun-core --features plugin-runtime plugin_runtime::tests`
