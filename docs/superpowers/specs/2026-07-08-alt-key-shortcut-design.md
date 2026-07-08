# Design Spec: Alt + Key Shortcut Selection

## 1. Background & Requirements
- **Goal**: Allow users to instantly select and commit clipboard history or snippet items using keyboard shortcuts (`Alt` + `1`..`9`, `0`, `A`..`Z`).
- **Scope**:
  - Dynamically map shortcut keys to the top visible items currently in the list box viewport (relative indexing based on `top_index`).
  - Intercept `WM_SYSKEYDOWN` (representing `Alt` + key presses) in the main message loop when Clipper is visible.
  - Draw a high-fidelity keycap label (e.g., `[1]`, `[A]`) on the left side of each visible list item using owner-draw (`WM_DRAWITEM`).
  - Keep folder and text margins adjusted appropriately to make space for the new keycap label.

---

## 2. Implementation Details

### 2.1. Shortcut Key Mappings
The characters are mapped in an intuitive order:
`1`, `2`, `3`, `4`, `5`, `6`, `7`, `8`, `9`, `0` (indices 0–9), followed by alphabetical keys `A`..`Z` (indices 10–35).

A helper function `get_shortcut_index(wparam: usize) -> Option<usize>` will map virtual keycodes to indices:
- `'1'`..`'9'` (`0x31`..`0x39`) -> `Some(wparam - 0x31)`
- `'0'` (`0x30`) -> `Some(9)`
- `'A'`..`'Z'` (`0x41`..`0x5A`) -> `Some(10 + (wparam - 0x41))`

---

### 2.2. Message Loop Interception
- **File**: [src/main.rs](file:///D:/Develop/clipper/src/main.rs)

**Changes in `main.rs`:**
Inside the `GetMessageW` loop, detect when the Clipper dialog is visible and `WM_SYSKEYDOWN` is received:
```rust
            } else if is_visible && msg.message == win32::WM_SYSKEYDOWN {
                if let Some(shortcut_idx) = get_shortcut_index(msg.wparam) {
                    let top_index = {
                        let state_guard = lock_state();
                        state_guard.as_ref().map_or(0, |s| s.top_index)
                    };
                    let target_idx = top_index + shortcut_idx;
                    let item_count = {
                        let state_guard = lock_state();
                        state_guard.as_ref().map_or(0, |s| s.current_results.len())
                    };

                    if target_idx < item_count {
                        if let Some(SafeHWND(hwnd_listbox)) = LISTBOX_HWND.get() {
                            unsafe {
                                win32::SendMessageW(*hwnd_listbox, win32::LB_SETCURSEL, target_idx, 0);
                            }
                            ui::on_select();
                        }
                        continue;
                    }
                }
            }
```

---

### 2.3. Keycap Drawing in ListBox
- **File**: [src/wndproc.rs](file:///D:/Develop/clipper/src/wndproc.rs)

**Changes in `wndproc.rs` (inside `WM_DRAWITEM` handler):**
1. Define the character mapping slice:
   ```rust
   const SHORTCUT_CHARS: &[char] = &[
       '1', '2', '3', '4', '5', '6', '7', '8', '9', '0',
       'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z'
   ];
   ```
2. Determine if the current `item_id` is within the viewport's shortcut keys range:
   ```rust
   let top_index = unsafe {
       win32::SendMessageW(dis.hwnd_item, win32::LB_GETTOPINDEX, 0, 0)
   } as usize;
   let relative_idx = dis.item_id as usize - top_index;
   let shortcut_char_opt = if relative_idx < SHORTCUT_CHARS.len() {
       Some(SHORTCUT_CHARS[relative_idx])
   } else {
       None
   };
   ```
3. Adjust layout offsets:
   - Introduce `shortcut_area_width = (24.0 * scale) as i32`.
   - Update `text_left_margin` to shift right:
     ```rust
     let text_left_margin = if has_icon {
         shortcut_area_width + (34.0 * scale) as i32
     } else {
         shortcut_area_width + (12.0 * scale) as i32
     };
     ```
   - Shift the icon rendering position `icon_x` rightward by `shortcut_area_width`.
4. Draw keycap label (e.g. `[1]` or `[A]`) at the left margin:
   - Draw a subtle rounded keycap border/fill in `colors.dim_text_color` (low opacity) or a dedicated grey brush.
   - Text color matches theme's normal text or accent color.

---

## 3. Test Plan
1. **Manual Verification**:
   - Run Clipper. Verify that on the left side of the list box, items `1` through `10` show keycaps `1`..`0`, and subsequent items show `A`..`Z` (up to `max_rows`).
   - Press `Alt + 1`. Verify the first item is selected and copied/expanded immediately.
   - Scroll down using the arrow keys or mouse wheel. Verify that the keycap labels stay locked on the screen's relative positions (i.e. the new top item gets keycap `1`).
   - Open a nested folder in snippets mode. Verify that folder items show keycaps too, and pressing the shortcut enters the folder.
