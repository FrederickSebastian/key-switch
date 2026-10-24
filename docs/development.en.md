# Key Switch Development Guide

[简体中文](./development.md) · English · [繁體中文](./development.zh-TW.md)

## 1. Prerequisites

- Node.js 22 LTS or a later compatible version
- npm
- The Rust stable toolchain
- Platform dependencies required by Tauri 2

Windows also requires Microsoft C++ Build Tools and WebView2 Runtime.

## 2. Run the project

From the project root:

```bash
npm install
npm run tauri:dev
```

To work on the frontend only:

```bash
npm run dev
```

Browser mode does not provide Tauri capabilities such as the system credential store, clipboard, application data directory, or persistent application settings. Settings changed in browser mode only last for that session.

## 3. Common checks

```bash
# Type-check and build the frontend for production
npm run build

# Frontend navigation and ordering tests
npm run test:frontend

# Format, compile, and test Rust
cargo fmt --all -- --check
cargo check --workspace --locked
cargo test --workspace --locked
```

Changes to frontend/backend contracts, the settings file, or security-sensitive logic must pass both the frontend build and Rust tests before being submitted.

### Versioning and releases

The current application version is `1.1.0`. Version changes must update `package.json`, root package versions in `package-lock.json`, `src-tauri/Cargo.toml`, the application entry in `Cargo.lock`, `src-tauri/tauri.conf.json` including the Windows WiX version, all four README badges, and the default release workflow tag.

UI fallback versions come from `package.json`; Rust request headers use `CARGO_PKG_VERSION` instead of separate version strings. The running desktop version still comes from Tauri.

CI and release builds use Node.js 22 and run `npm run test:frontend`. The release tag must match the application version. `.github/release-notes.md` contains the current release notes, and the release workflow creates drafts only. Commit, push, tag, and publish actions require explicit release instructions.

## 4. Architecture and directory responsibilities

| Directory | Responsibility |
| --- | --- |
| `src/views/` | Main page, provider management, and categorized settings pages |
| `src/components/` | Reusable Vue components, dialogs, and base UI components |
| `src/api/` | TypeScript types and wrappers for Tauri commands |
| `src/stores/` | Pinia state, business operations, and the settings persistence queue |
| `src/i18n/` | Locale resolution, Vue I18n setup, error translation, and four message sets |
| `src/data/` | Non-sensitive static data such as the built-in provider catalog |
| `src-tauri/src/` | Desktop commands, windows, theme, updates, and system adapters |
| `crates/key-switch-core/` | Shared business core, persistence, credentials, and checks |
| `crates/key-switch-tui/` | Terminal interaction, background jobs, and clipboard adapter |
| `src-tauri/capabilities/` | Least-privilege Tauri capability declarations |
| `docs/` | Development documentation and README assets |

The frontend owns presentation, interaction, and non-sensitive state. Sensitive data, persistent files, external requests, and operating-system capabilities belong on the Rust side. New frontend code should keep the Vue `<script setup lang="ts">` style and complete type definitions.

### Page navigation and ordering

- The sidebar sizes itself using a hidden width guide for built-in Chinese and English names plus CSS inline-size containment; custom long names do not expand it. Both headings share a height. Provider type and platform URL follow the right heading; long URLs truncate with a full tooltip and remain clickable.
- Provider changes use short fades for the heading and Key content, with a slight content shift. Reduced motion disables fades and shifts. Leaving content ignores pointer interaction. The actions column reserves a fixed width for all four buttons, and masked cells keep table layout.
- The main page lists providers on the left and the selected provider's Keys on the right. Provider management and settings links sit at the bottom left.
- Provider selection lives only in the current Pinia session, without new data files or settings fields. The sidebar has no provider search and always shows the full list. The first provider is the default and the fallback when the selected provider is deleted. Key search terms live only in the page component.
- Sidebar ordering reuses pointer capture, the 4px activation threshold, and FLIP drop animations. The drag handle also supports `Alt + ↑ / ↓`; reduced motion skips drop animations.
- Ordering uses the existing `reorder_providers` command with the complete ID list. Further reordering is disabled while saving; failures restore the previous order and display a translated error.
- `/settings` redirects to `/settings/general`. Existing settings are grouped into General (language, appearance), Data & logs (storage directory, logs), and About (version, repository, updates), at `/settings/general`, `/settings/data`, and `/settings/about`. Back returns to the main page.
- Version and update installation status appear at the bottom of the sidebar. Settings continue using the existing Store queue; Rust contracts and runtime data structures are unchanged.
- Settings category pages share the main page heading height, text and icon sizes, with compact controls. Bottom sidebar navigation wraps when space is limited and preserves full labels in every language.

## 5. Local data and application settings

Runtime data is stored in Tauri's per-user application data directory:

| Content | Location | Notes |
| --- | --- | --- |
| Provider and key metadata | `key-switch-data.json` | Never contains complete API Keys |
| Application settings | `settings.json` | Non-sensitive preferences only |
| Complete API Keys | System credential store | Never written to JSON |
| Application logs | `logs/` | Must not contain complete keys or credentials |

Current settings file example:

```json
{
  "schemaVersion": 1,
  "localePreference": "system",
  "themePreference": "system"
}
```

`themePreference` accepts `system`, `light`, or `dark`. The theme is selected on the Settings page; `system` follows operating-system appearance changes in real time. The theme is applied to both WebView content and the native Tauri window title bar, and changes are persisted through the settings Store write queue.

Settings startup flow:

1. `src/main.ts` loads the settings Store before mounting Vue.
2. Rust reads and validates `settings.json`, creating defaults on the first run.
3. For upgraded users, the old `key-switch.locale` value is migrated only when the settings file is first created.
4. The legacy `localStorage` key is removed after a successful migration.
5. The language and theme are applied before Vue mounts, preventing a startup flash.

Settings writes are protected as follows:

- A frontend queue preserves the order of rapid consecutive changes.
- Rust's `SETTINGS_LOCK` prevents concurrent file writes.
- New content is written to `settings.json.tmp` and explicitly flushed to disk.
- The current file is renamed to `settings.json.bak` before the temporary file replaces it.
- On failure, Rust restores the previous file and the frontend rolls back to the most recently saved settings.
- On startup after an interrupted replacement, a fully written temporary file is preferred; otherwise the backup is restored.

When adding a setting, update all of the following:

1. Rust `AppSettings` or the relevant nested settings type.
2. Rust `Default` values, range validation, and any required migration.
3. TypeScript `AppSettings`.
4. `normalizeSettings()` in `src/stores/settings.ts`.
5. The settings UI, all four message sets, and relevant tests.

The frontend merges partial changes with `settingsStore.updateSettings()`, but every command and file write contains the complete settings object. A compatible field with a default can keep the current `schemaVersion`. Removing a field, changing its type, or changing its meaning requires a version bump and an explicit migration. Do not change only the version number: unsupported versions are intentionally rejected.

Logging behavior: native business events are written to `logs/key-switch.log`, with one rotated 1 MB backup. The frontend uses a single logging command for Vue runtime errors, unhandled Promise rejections, resource errors, and failed Tauri commands. Client entries accept only allow-listed levels and event names; details have newlines removed, are truncated, and redact common credential fields. Users can open the log directory from the Settings page for troubleshooting.

## 6. Internationalization rules

Supported preferences:

- `zh-CN`: Simplified Chinese
- `zh-TW`: Traditional Chinese
- `en-US`: English
- `ja-JP`: Japanese
- `system`: Follow the operating system

If the system locale cannot be read, the app falls back to Simplified Chinese. A readable but unsupported non-Chinese system locale falls back to English.

Development rules:

- Every user-visible string must use Vue I18n, including buttons, toasts, errors, placeholders, `title`, `aria-label`, and image `alt` text.
- `src/i18n/messages/zh-CN.ts` defines the message shape; every other locale must satisfy the same `MessageSchema`.
- Add or remove keys in all four message sets and run `npm run build` to verify their structure.
- Use semantic, domain-based keys instead of source-language text as keys.
- Custom provider names and names already stored in user data are user content: never translate or silently rename them.
- When a built-in provider is added, Chinese locales use its Chinese catalog name. English, Japanese, and other non-Chinese locales use its English name. The displayed name is persisted at creation time.
- The provider catalog maintains stable IDs plus Chinese and English names; changing the locale must not rewrite stored records.
- Animated components must support `prefers-reduced-motion`. Custom controls such as selects must retain keyboard behavior and accessibility semantics.

## 7. Tauri commands and error handling

- Call `invoke` through `src/api/app.ts`; do not scatter command strings across page components.
- Define native commands with `#[tauri::command]` and register them in `tauri::generate_handler!` in `src-tauri/src/lib.rs`.
- Keep command arguments and return types synchronized between Rust and TypeScript.
- Rust exposes only stable, non-sensitive error codes to the UI; raw internal errors must not be displayed to users.
- `src/i18n/errors.ts` maps error codes to translated messages.
- Business logic must never determine state by matching Chinese or English error text.
- A new error code requires a Rust mapping, a frontend mapping, all four translations, and tests.

## 8. Security rules

- Complete API Keys may only be read and handled by Rust. They must never enter logs, URLs, persistent DOM state, frontend Stores, `localStorage`, or `settings.json`.
- List commands return masked values and non-sensitive metadata only. Secrets are stored in the system credential store.
- Copying, decrypting, and validation happen on the Rust side, with plaintext lifetimes kept as short as practical.
- External validation may only contact explicitly allowed provider endpoints and must enforce timeouts, response-size limits, and clear error categories.
- The settings file stores non-sensitive preferences only. Review every new settings field for sensitivity.
- Review `src-tauri/capabilities/` before adding a Tauri plugin or operating-system capability, and keep permissions minimal.
- Never commit real API Keys in source, fixtures, Issues, pull requests, logs, or screenshots.

### Custom provider key checks

- Custom providers default to `none`. Existing records without a check configuration must also fall back to `none` and the unsupported status.
- `platformUrl` is only for opening the management console and must not be used as a check endpoint. Check configuration is stored separately in provider metadata and currently supports OpenAI-compatible, Bearer token, and API key header modes.
- A check configuration stores only an HTTPS endpoint, authentication mode, and a non-sensitive header name. The full key is still read temporarily from the system credential store by Rust only.
- Custom check URLs must not contain credentials, query parameters, or fragments. Loopback, private, link-local, reserved addresses, and hostnames resolving to them are rejected, and HTTP redirects remain disabled.
- Check requests do not read or log response bodies. Rate limits, timeouts, network failures, server failures, and endpoint errors must use distinct stable, non-sensitive result codes and must not be treated as invalid keys.
- Changing a provider check configuration must clear previous key check results. No network request may be made when checking is unsupported.

## 9. Change checklist

- UI changes: verify all four locales, long text, keyboard interaction, and reduced motion.
- Settings changes: verify defaults, validation, full-object merging, rollback, and old-file compatibility.
- Rust command changes: verify frontend/backend types, command registration, and structured error codes.
- Data model changes: never silently rewrite existing names, notes, or other user data.
- Security changes: confirm sensitive values cannot reach frontend persistence, settings files, or logs.
- Documentation changes: keep `development.md`, `development.en.md`, and `development.zh-TW.md` synchronized.
- At minimum, run `npm run build`, `cargo fmt --all -- --check`, `cargo check --workspace --locked`, and `cargo test --workspace --locked`.

## 10. Shared core and TUI

The root Cargo workspace includes `src-tauri`, `crates/key-switch-core` and `crates/key-switch-tui`, with one root `Cargo.lock` and root `target/` output. The core owns models, transactions, persistence, credentials, checks and safe logs without Tauri or terminal dependencies. Desktop commands keep their protocol, window, theme and updater adapters. Non-graphical catalog fields come from `crates/key-switch-core/provider-catalog.json`; Vue maintains image mappings.

Build and run from the repository root:

```bash
cargo run -p key-switch-tui --locked
cargo build -p key-switch-tui --release --locked
cargo test -p key-switch-core -p key-switch-tui --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
node scripts/check-versions.mjs
```

The TUI supports local desktop terminals on Windows x64 and Linux x64. Building it does not require WebView. Linux runtime requires an available, unlocked Secret Service and X11 / Wayland clipboard; Windows uses Credential Manager. Both retain the keyring v1 native default backend and service `com.app.key-switch`, with no plaintext fallback. SSH, headless Linux, WSL, macOS TUI, image editing, settings editing and automatic updates are unsupported. arboard enables `wayland-data-control`; the compositor needs this protocol or a working XWayland clipboard. The clipboard connection stays alive during the session; retention after exit depends on the desktop clipboard manager.

The Windows MSVC TUI build requires the Microsoft Visual C++ x64 runtime (including `VCRUNTIME140.dll`); the standalone archive does not bundle a runtime installer.

Paths match Tauri: `com.app.key-switch` under Windows Roaming AppData, or `$XDG_DATA_HOME/com.app.key-switch` on Linux, falling back to `~/.local/share/com.app.key-switch`. The core accepts an explicit directory; the TUI uses the same dirs resolver as Tauri and has no separate data-directory option.

Startup acquires an OS nonblocking exclusive lock on `key-switch.lock` before settings, business recovery or credentials. Desktop sets `create: false` and creates its window after acquiring the lock; TUI enters raw mode afterward. A second instance displays a localized message and exits; unavailable locks cause startup failure. Never remove, truncate or replace the lock file. The OS releases the lock on exit or crash. **Old desktop builds do not participate: upgrade and close old instances before using the TUI.**

Business operations use one process transaction mutex; settings and logs retain separate locks. Network checks run outside transactions and merge only matching Key identities, credential references, provider configuration and in-memory check generations. Late results preserve remarks, additions, deletions and ordering. Newer checks, credential replacements and configuration changes invalidate older checks. Reordering requires an exact permutation of all current provider IDs.

Metadata uses synchronized temporary files and backup replacement. Recovery validates JSON, rejects a corrupt main file and restores a complete temporary file or backup when the main file is missing. Unix also synchronizes directories; Windows synchronizes files and uses same-directory renames without promising directory-entry durability after power loss. `key-switch-transaction.json` contains only before/after metadata and credential IDs. Secrets and backups remain in the keyring. Creation and replacement use random credential IDs while retaining old Key IDs. Deletion and replacement stage keyring backups, attempt rollback on failure and recover or clean up after interruption based on committed metadata. Failed rollback or recovery returns a specific error, preserves recovery files and blocks further business operations. Do not delete recovery files manually. Permanent keyring failure prevents guaranteed automatic recovery.

Keys: arrows select, Tab / Enter switches panes, `n` creates, `e` edits, `d` confirms deletion, `b` adds built-ins, `/` searches, `s` toggles session Key sorting by remark, `Alt+↑/↓` persists provider order (disabled during search), `c` copies, `t` checks a Key, `a` checks the provider, `r` reloads, and `q` / Escape exits or clears search. Forms use Tab / arrows for fields, left/right for cursor movement, Enter to save and Escape to cancel. Secret fields are hidden and confined to temporary input/operation buffers. Copy returns only completion status. Background jobs disable mutations while busy. The TUI reads the shared language preference for Simplified Chinese, Traditional Chinese, English and Japanese without modifying theme or other settings. RAII and a panic hook restore the terminal; forced external termination cannot guarantee destructors run.

CI retains desktop/navigation checks and adds standalone Windows/Linux core and TUI checks without WebView. Release preparation creates one draft; desktop platforms remain and TUI archives are `key-switch-tui-v<version>-windows-x64.zip` and `key-switch-tui-v<version>-linux-x64.tar.gz`, with no TUI updater manifest. The project remains at `1.1.0`; update all version locations before the actual release. Feature branches do not publish directly.

Automated tests use mock credentials and child processes without accessing real credentials. Windows 10/11 and Linux X11/Wayland still require real validation of alternating frontends, keyrings, clipboard, Chinese input, all four languages and terminal restoration. Compilation and mocks do not replace platform acceptance.

Run `node scripts/sync-native-errors.mjs` to generate native error resources from all four desktop translations; `npm run test:frontend` checks parity.
