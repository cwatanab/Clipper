# Design Spec: Snippet Folder Fallback & Sorting Option

## 1. Background & Requirements
- **Goal**:
  1. Ensure that choosing "Open Snippets Folder" automatically creates the folder if it does not exist, preventing fallback errors in Windows Explorer.
  2. Allow snippets to be displayed in the order they are defined inside TOML files (the default behavior) rather than always sorting them alphabetically, while adding a configurable option `sort_snippets` to enable alphabetical sorting.
- **Scope**:
  - Automatically create `%APPDATA%\clipper\snippets` folder prior to spawning `explorer.exe` when handling command ID `1005`.
  - Enable the `preserve_order` feature on the `toml` crate to preserve parsing order of TOML tables.
  - Introduce `sort_snippets` configuration option (boolean, default: `false`).
  - Modify UI sorting logic in [src/filter.rs](file:///D:/Develop/clipper/src/filter.rs) to only sort directories and snippets when `sort_snippets = true`.

---

## 2. Implementation Details

### 2.1. Folder Creation Fallback
- **File**: [src/ui.rs](file:///D:/Develop/clipper/src/ui.rs)

**Changes in `ui.rs`:**
Modify the command handler for ID `1005` to create directory recursively:
```rust
    } else if cmd == 1005 {
        let path = util::get_app_dir().join("snippets");
        let _ = std::fs::create_dir_all(&path);
        let _ = std::process::Command::new("explorer").arg(path).spawn();
    }
```

---

### 2.2. TOML Parser Order Preservation
- **File**: [Cargo.toml](file:///D:/Develop/clipper/Cargo.toml)

**Changes in `Cargo.toml`:**
Enable `preserve_order` feature on `toml` crate to ensure that `toml::Value::Table` maintains the key definition order:
```toml
toml = { version = "0.8", features = ["preserve_order"] }
```

---

### 2.3. Sorting Config Option
- **Files**:
  - `config.toml`
  - [src/config.rs](file:///D:/Develop/clipper/src/config.rs)

**Changes in `config.rs`:**
- Add `sort_snippets: bool` field to `Config`.
- Implement `default_sort_snippets() -> bool` returning `false`.
- Include `sort_snippets: false` in `impl Default for Config`.

---

### 2.4. Conditional UI Sorting
- **File**: [src/filter.rs](file:///D:/Develop/clipper/src/filter.rs)

**Changes in `filter.rs`:**
Retrieve `sort_snippets` from the config state.
- **Subdirectories**: Only call `folders.sort()` if `sort_snippets` is `true`.
- **Snippets**: Only call `local_snippets.sort_by(|a, b| a.1.cmp(&b.1))` if `sort_snippets` is `true`.

---

## 3. Test Plan
1. **Unit Tests**:
   - Add unit test assertions in [src/config.rs](file:///D:/Develop/clipper/src/config.rs) to verify `sort_snippets` parses correctly and defaults to `false`.
2. **Manual Verification**:
   - Delete the `snippets` directory. Select "Open Snippets Folder" in Clipper and verify that the directory is re-created automatically.
   - Define snippets in `snippets.toml` out of alphabetical order. Verify they display in the exact TOML sequence.
   - Set `sort_snippets = true` in `config.toml`, reload configuration, and verify they now display in alphabetical order.
