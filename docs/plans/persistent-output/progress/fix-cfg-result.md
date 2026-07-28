# fix-cfg-result

## 2026-03-28 taiji feature 修复验证

### 检查文件状态

| # | 文件 | 状态 |
|---|---|---|
| 1 | `src/crates/contracts/core-types/Cargo.toml` | `taiji = []` 已存在 ✅ |
| 2 | `src/crates/contracts/runtime-ports/Cargo.toml` | `taiji = []` 已存在 ✅ |
| 3 | `src/crates/services/services-core/Cargo.toml` | `taiji = []` 已存在 ✅ |
| 4 | `src/crates/execution/agent-runtime/Cargo.toml` | `taiji = [...]` 含传播链 ✅ |
| 5 | `src/crates/execution/tool-contracts/Cargo.toml` | `taiji = []` 已存在 ✅ |
| 6 | `src/crates/assembly/core/Cargo.toml` | `taiji = [...]` 含传播链，`product-full` 不含 taiji ✅ |

结论：merge 未覆盖 taiji feature，6 个文件均完好。

### cargo check 编译验证

```powershell
$env:SHERPA_ONNX_LIB_DIR = "$env:USERPROFILE\.sherpa-onnx\v1.13.4\sherpa-onnx-v1.13.4-win-x64-static-MT-Release-lib\lib"
cargo check -p bitfun-core --features taiji 2>&1 | Select-Object -Last 3
```

输出：
```
    Compiling bitfun-core v0.2.14 (E:\finance-trading\lvpa\software\taiji\src\crates\assembly\core)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 41.65s
```

**结果：通过 ✅**（exit code 0）
