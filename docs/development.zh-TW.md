# Key Switch 二次開發文件

[简体中文](./development.md) · [English](./development.en.md) · 繁體中文

## 1. 開發環境

- Node.js 22 LTS 或更新的相容版本
- npm
- Rust stable 工具鏈
- Tauri 2 對應的平台依賴

Windows 另外需要 Microsoft C++ Build Tools 和 WebView2 Runtime。

## 2. 啟動專案

在專案根目錄執行：

```bash
npm install
npm run tauri:dev
```

如果只需要除錯前端介面，可以執行：

```bash
npm run dev
```

瀏覽器模式不具備系統密鑰庫、剪貼簿、應用程式資料目錄和持久化設定等 Tauri 能力。瀏覽器模式中的設定只在目前工作階段有效。

## 3. 常用檢查

```bash
# 前端型別檢查與生產建置
npm run build

# 前端導覽與排序測試
npm run test:frontend

# Rust 格式、編譯與測試
cargo fmt --all -- --check
cargo check --workspace --locked
cargo test --workspace --locked
```

提交涉及前後端協定、設定檔或安全邏輯的變更前，必須同時執行前端建置與 Rust 測試。

### 版本與發布

目前應用程式版本為 `1.1.0`。更新版本時同步 `package.json`、`package-lock.json` 根套件版本、`src-tauri/Cargo.toml`、`Cargo.lock` 的應用套件版本、`src-tauri/tauri.conf.json` 及 Windows WiX 版本，並更新四份 README 版本徽章與發布流程預設標籤。

介面版本回退值讀取 `package.json`，Rust 請求標頭使用 `CARGO_PKG_VERSION`；不再個別維護版本字串。實際桌面版本仍以 Tauri 回傳的資訊為準。

CI 與發布建置使用 Node.js 22，執行 `npm run test:frontend`。發布標籤必須與應用版本一致；`.github/release-notes.md` 儲存目前版本的發布說明，發布流程只建立草稿。程式碼提交、推送、建立標籤與發布應依明確的發布指令執行。

## 4. 架構與目錄職責

| 目錄 | 說明 |
| --- | --- |
| `src/views/` | 主頁面、供應商管理與設定分類頁面 |
| `src/components/` | 可重用 Vue 元件、對話框和基礎 UI 元件 |
| `src/api/` | Tauri 命令的 TypeScript 型別與呼叫封裝 |
| `src/stores/` | Pinia 狀態、業務操作和設定持久化佇列 |
| `src/i18n/` | 語言解析、Vue I18n 初始化、錯誤翻譯和四套語言資源 |
| `src/data/` | 內建供應商目錄等非敏感靜態資料 |
| `src-tauri/src/` | 桌面命令、視窗、主題、更新與系統適配 |
| `crates/key-switch-core/` | 共用業務核心、持久化、認證與檢測 |
| `crates/key-switch-tui/` | 終端互動、背景工作與剪貼簿適配 |
| `src-tauri/capabilities/` | Tauri 最小權限宣告 |
| `docs/` | 專案開發文件和 README 資源 |

前端負責顯示、互動和非敏感狀態；敏感資料、持久化檔案、外部請求和系統能力必須放在 Rust 側。新增前端程式碼時，保持 Vue `<script setup lang="ts">` 風格和完整型別定義。

### 頁面導覽與排序

- 側欄透過隱藏的內建中英文名稱寬度基準與 CSS 行內尺寸包含自動定寬，不依自訂長名稱擴展。兩側標題共用高度；供應商類型與平台網址置於右側標題後，長網址截斷但保留完整提示與開啟入口。
- 切換供應商時，標題與 Key 內容使用短淡入淡出過渡；內容附帶輕微位移，減少動畫模式停用過渡與位移。退出中的內容暫停指標操作。操作欄保留四個按鈕所需固定寬度，遮罩儲存格維持表格佈局。
- 主頁面採用左側供應商、右側目前供應商 Key 的佈局；供應商管理與設定入口位於左下角。
- 供應商選取狀態僅保留在 Pinia 目前工作階段，不新增資料檔案或設定欄位。左側不提供供應商搜尋，始終顯示完整清單；首次載入預設選取第一個供應商，已選供應商遭刪除時回退到首項。右側 Key 搜尋關鍵字僅保留在頁面元件中。
- 左側供應商排序沿用指標捕獲、4px 啟動門檻與 FLIP 落位動畫，並支援在拖曳手柄上使用 `Alt + ↑ / ↓` 調整順序。減少動畫模式略過落位動畫。
- 排序仍透過既有 `reorder_providers` 命令儲存完整 ID 順序；儲存期間禁止重複排序，失敗時恢復原順序並顯示翻譯後的錯誤。
- `/settings` 預設進入 `/settings/general`。設定依現有功能分為一般（語言、外觀）、資料與日誌（儲存目錄、日誌）、關於（版本、儲存庫、更新），分別對應 `/settings/general`、`/settings/data`、`/settings/about`；返回按鈕回到主頁面。
- 版本與更新安裝狀態顯示於側欄底部；設定儲存仍使用既有 Store 佇列，Rust 協定與執行階段資料結構保持不變。
- 設定分類頁沿用主頁面的標題高度、文字與圖示尺寸，控制項維持緊湊。側欄底部導覽在剩餘寬度不足時自動換行，完整保留各語言的標籤。

## 5. 本機資料與設定

執行階段資料位於 Tauri 的每位使用者應用程式資料目錄：

| 內容 | 儲存位置 | 說明 |
| --- | --- | --- |
| 供應商與 Key 中繼資料 | `key-switch-data.json` | 不包含完整 API Key |
| 應用程式設定 | `settings.json` | 只保存非敏感偏好 |
| 完整 API Key | 系統密鑰庫 | 不寫入 JSON 檔案 |
| 執行記錄 | `logs/` | 不得記錄完整 Key 或其他憑據 |

目前設定檔範例：

```json
{
  "schemaVersion": 1,
  "localePreference": "system",
  "themePreference": "system"
}
```

`themePreference` 支援 `system`、`light` 和 `dark`。主題在設定頁面選擇；`system` 會即時跟隨作業系統的淺色或深色外觀。主題會同時套用到 WebView 內容與 Tauri 原生視窗標題列，變更透過設定 Store 的寫入佇列持久化。

設定啟動流程：

1. `src/main.ts` 在掛載 Vue 前呼叫設定 Store。
2. Rust 讀取並驗證 `settings.json`；首次執行時建立預設檔案。
3. 升級使用者如果仍有舊的 `key-switch.locale`，只在首次建立設定檔時遷移。
4. 成功遷移後刪除舊 `localStorage` 鍵。
5. 設定回傳前端後先套用語言和主題，再掛載介面，避免啟動時閃切。

設定寫入包含以下保護：

- 前端寫入佇列確保快速連續修改依序提交。
- Rust `SETTINGS_LOCK` 防止同時寫入檔案。
- 新內容先寫入 `settings.json.tmp` 並明確同步至磁碟。
- 原檔暫時改名為 `settings.json.bak`，再由暫存檔替換正式檔案。
- 寫入失敗時恢復上一個檔案，前端同時回復到最近一次成功設定。
- 啟動時若偵測到中斷狀態，會優先恢復已完整寫入的暫存檔，否則恢復備份。

新增設定項目時必須同步修改：

1. Rust `AppSettings` 或對應的巢狀設定結構。
2. Rust `Default` 預設值、欄位範圍驗證和必要的遷移邏輯。
3. TypeScript `AppSettings`。
4. `src/stores/settings.ts` 中的 `normalizeSettings()`。
5. 設定介面、四套翻譯和相關測試。

前端透過 `settingsStore.updateSettings()` 合併局部修改，但每次傳送與寫入的是完整設定物件。只新增帶有預設值的相容欄位時，可以保留目前的 `schemaVersion`；刪除欄位、修改型別或改變含義時，必須提升版本並實作舊版本遷移。不能只修改版本數字，目前實作會拒絕不支援的版本。

記錄行為：Rust 側業務事件寫入 `logs/key-switch.log`，檔案達到 1 MB 時保留一個輪替備份；前端透過統一記錄命令回報 Vue 執行期例外、未處理的 Promise rejection、資源錯誤和 Tauri 命令失敗。前端記錄只接受白名單等級與事件名稱，詳細內容會移除換行、截斷並遮蓋常見憑據欄位。使用者可從設定頁開啟記錄目錄進行排查。

## 6. 國際化規範

目前支援：

- `zh-CN`：簡體中文
- `zh-TW`：繁體中文
- `en-US`：英文
- `ja-JP`：日文
- `system`：跟隨系統

無法讀取系統語言時回退到簡體中文；能夠讀取但尚未支援的非中文系統語言使用英文。

開發要求：

- 所有使用者可見文案都必須使用 Vue I18n，包括按鈕、Toast、錯誤、預留位置、`title`、`aria-label` 和圖片 `alt`。
- 簡體中文資源 `src/i18n/messages/zh-CN.ts` 是型別結構基準，其他語言必須符合相同的 `MessageSchema`。
- 新增或刪除翻譯鍵時，必須同步四套語言資源，並執行 `npm run build` 檢查鍵結構。
- 不要把中文原文當作翻譯鍵；使用依領域組織的語意鍵。
- 自訂供應商名稱和已寫入使用者資料的名稱屬於使用者內容，不翻譯、不自動改名。
- 新增內建供應商時，中文介面使用中文目錄名稱；英文、日文及其他非中文介面使用英文名稱，並將當時顯示的名稱寫入資料檔。
- 內建供應商目錄只維護穩定 ID、中文名和英文名，不因目前語言改寫已儲存記錄。
- 動畫元件必須支援 `prefers-reduced-motion`；自訂選擇框等控制項必須保留鍵盤操作和無障礙語意。

## 7. Tauri 命令與錯誤處理

- 前端統一透過 `src/api/app.ts` 呼叫 `invoke`，不要在頁面元件中散落命令字串。
- 新增 Rust 命令使用 `#[tauri::command]`，並在 `src-tauri/src/lib.rs` 的 `tauri::generate_handler!` 中註冊。
- 命令參數和回傳結構必須同步維護 Rust 與 TypeScript 型別。
- Rust 對介面只回傳穩定、非敏感的錯誤碼；底層原始錯誤不得直接顯示給使用者。
- 前端透過 `src/i18n/errors.ts` 將錯誤碼對應到翻譯鍵。
- 業務邏輯不得透過比對中文或英文錯誤文字判斷狀態。
- 新增錯誤碼時，同步更新 Rust 分類、前端錯誤碼對應、四套語言文案和測試。

## 8. 安全約定

- 完整 API Key 只能由 Rust 側讀取和處理，不得進入日誌、URL、DOM 持久狀態、前端 Store、`localStorage` 或 `settings.json`。
- 清單命令只回傳掩碼和非敏感中繼資料；敏感值保存到系統密鑰庫。
- 複製、解密和檢測必須在 Rust 側完成，並盡量縮短明文存活時間。
- 外部檢測只能存取明確允許的供應商端點，並設定逾時、回應內容大小上限和錯誤分類。
- 設定檔只保存非敏感應用程式偏好；未來新增設定欄位前必須進行敏感性審查。
- 新增 Tauri 外掛或系統能力前，檢查 `src-tauri/capabilities/` 並維持最小權限。
- 不得在原始碼、測試資料、Issue、PR、日誌或截圖中提交真實 API Key。

### 自訂供應商 Key 檢查

- 自訂供應商預設使用「不檢查」，舊資料缺少檢查設定時也必須回退到「不檢查」與「不支援檢查」狀態。
- 平台管理網址 `platformUrl` 僅用於開啟管理後台，不得作為檢查端點。檢查設定獨立儲存在供應商中繼資料，目前支援 OpenAI 相容、Bearer Token 與 API Key Header 三種方式。
- 檢查設定只保存 HTTPS 端點、驗證方式與非敏感 Header 名稱；完整 Key 仍僅由 Rust 側從系統密鑰庫暫時讀取。
- 自訂檢查網址不得包含憑據、查詢參數或片段，預設拒絕回環、區域網路、鏈路本地、保留位址以及解析到這些位址的網域，並繼續禁止 HTTP 重新導向。
- 檢查請求不讀取或記錄回應本文。限流、逾時、網路故障、伺服器故障與端點錯誤必須使用穩定且非敏感的結果碼區分，不得誤判為 Key 無效。
- 修改供應商檢查設定後，必須清除該供應商既有 Key 的舊狀態；沒有檢查能力時不得發起網路請求。

## 9. 變更檢查清單

- 介面變更：檢查四種語言、長文字、鍵盤操作和減少動畫設定。
- 設定變更：檢查預設值、驗證、完整物件合併、失敗回復和舊檔相容性。
- Rust 命令變更：檢查前後端型別、命令註冊和結構化錯誤碼。
- 資料模型變更：不得無提示改寫使用者既有名稱、備註或其他業務資料。
- 安全變更：確認敏感資訊不會寫入前端持久化、設定檔或日誌。
- 文件變更：同步 `development.md`、`development.en.md` 和 `development.zh-TW.md`。
- 完成後至少執行 `npm run build`、`cargo fmt --all -- --check`、`cargo check --workspace --locked` 和 `cargo test --workspace --locked`。

## 10. 共用核心與 TUI

根 Cargo workspace 包含 `src-tauri`、`crates/key-switch-core` 與 `crates/key-switch-tui`，共用根 `Cargo.lock`，產物位於根 `target/`。核心負責模型、交易、持久化、系統金鑰庫、網路檢測及安全日誌，不依賴 Tauri 或終端函式庫；桌面保留命令協定、視窗、主題及更新適配。非圖形供應商目錄來自 `crates/key-switch-core/provider-catalog.json`，Vue 維護圖片映射。

在儲存庫根目錄獨立建置與執行：

```bash
cargo run -p key-switch-tui --locked
cargo build -p key-switch-tui --release --locked
cargo test -p key-switch-core -p key-switch-tui --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
node scripts/check-versions.mjs
```

TUI 支援 Windows x64 與 Linux x64 本機桌面終端，獨立建置無需 WebView。Linux 執行需要可用且已解鎖的 Secret Service 與 X11 / Wayland 剪貼簿，Windows 使用系統認證管理員。沿用 keyring v1 原生預設後端及 service `com.app.key-switch`，沒有明文回退。不支援 SSH、無桌面 Linux、WSL、macOS TUI、圖片編輯、設定編輯或自動更新。arboard 啟用 `wayland-data-control`；compositor 必須提供此協定或可用的 XWayland 剪貼簿。執行期間保持剪貼簿連線，結束後的保留由桌面剪貼簿管理員決定。

Windows MSVC 建置的 TUI 需要 Microsoft Visual C++ x64 執行階段（包括 `VCRUNTIME140.dll`）；獨立壓縮檔不包含執行階段安裝程式。

兩端資料路徑一致：Windows Roaming AppData 下的 `com.app.key-switch`；Linux 為 `$XDG_DATA_HOME/com.app.key-switch`，未設定時為 `~/.local/share/com.app.key-switch`。核心接受明確目錄，TUI 使用與 Tauri 相同的 dirs 解析規則，沒有獨立資料目錄選項。

啟動先取得 `key-switch.lock` 的 OS 非阻塞獨佔鎖，再初始化設定、還原業務檔案與存取認證。桌面設定 `create: false`，取得鎖後才建立主視窗；TUI 之後才進入原始模式。第二個執行個體顯示本地化提示並結束；鎖不可用則回報啟動失敗。不刪除、截斷或替換鎖檔，正常結束或崩潰由 OS 釋放鎖。**舊版桌面沒有此協定，使用 TUI 前必須升級並關閉舊執行個體。**

業務操作共用單進程交易鎖，設定與日誌保留各自鎖。網路檢測在交易外執行，完成後核對 Key 身分、認證引用、供應商設定與進程內檢測代次，只合併有效結果。舊快照不覆蓋備註、新增、刪除或順序；新檢測、替換認證與修改設定使舊結果失效。排序必須提交全部供應商 ID 的無重複完整排列。

中繼資料使用同步暫存檔及備份替換，還原驗證 JSON，拒絕損壞的主檔；主檔缺失時選擇完整暫存檔或備份。Unix 另同步目錄；Windows 同步檔案並在同目錄重新命名，不承諾斷電時目錄項目的持久性。`key-switch-transaction.json` 只記錄非敏感前後中繼資料及認證 ID，完整 Key 與備份只在金鑰庫。新增與替換使用隨機認證 ID，保留舊 Key ID 相容；刪除與替換先在金鑰庫備份。失敗嘗試復原，中斷後依中繼資料提交狀態清理或還原。復原或還原失敗回傳明確錯誤，保留還原檔案並阻擋後續業務操作，請勿手動刪除。金鑰庫永久失效時無法保證自動還原。

快捷鍵：方向鍵選擇、Tab / Enter 切換、`n` 新增、`e` 編輯、`d` 刪除確認、`b` 內建供應商、`/` 搜尋、`s` 切換依備註的 Key 顯示排序、`Alt+↑/↓` 儲存供應商順序（搜尋時停用）、`c` 複製、`t` 檢測 Key、`a` 檢測供應商、`r` 重試載入、`q` / Escape 結束或清除搜尋。表單使用 Tab / 方向鍵切換欄位、左右移動游標、Enter 儲存與 Escape 取消。Key 隱藏顯示，只保留於短暫輸入與操作緩衝區；複製只回傳完成狀態。工作在背景執行，忙碌時停用修改。TUI 讀取共用語言偏好，支援簡中、繁中、英文、日文，不修改主題或其他設定。終端守衛與 panic hook 還原終端；外部強制終止無法保證執行解構。

CI 保留桌面與導覽回歸，新增 Windows / Linux 獨立核心與 TUI 檢查，無 WebView 依賴。發布流程先建立一份草稿 Release，保留桌面平台產物；TUI 上傳 `key-switch-tui-v<版本>-windows-x64.zip` 或 `key-switch-tui-v<版本>-linux-x64.tar.gz`，不產生 updater 清單。目前仍為 `1.1.0`，實際發布前依統一版本檢查更新，功能分支不直接發布。

自動測試使用模擬認證及子進程，不讀取真實認證。Windows 10 / 11 與 Linux X11 / Wayland 的兩端交替、金鑰庫、剪貼簿、中文輸入、四語言與終端還原仍需實機驗收；編譯與模擬測試不能替代。

原生錯誤翻譯透過 `node scripts/sync-native-errors.mjs` 從桌面四套翻譯產生，`npm run test:frontend` 檢查兩端一致性。
