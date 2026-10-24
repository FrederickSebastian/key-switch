use key_switch_core::Locale;
pub fn text(locale: Locale, key: &str) -> &'static str {
    let values = match key {
        "providers" => ["供应商", "供應商", "Providers", "プロバイダー"],
        "keys" => ["API Key", "API Key", "API Keys", "API キー"],
        "empty" => ["暂无数据。按 n 新建，b 添加内置供应商。", "尚無資料。按 n 新增，b 加入內建供應商。", "No entries. n: new, b: built-in provider.", "項目なし。n: 新規、b: 組み込みプロバイダー。"],
        "loading" => ["处理中…", "處理中…", "Working…", "処理中…"],
        "ready" => ["完成", "完成", "Done", "完了"],
        "help" => ["↑↓ 选择  Tab 切换  n 新建  e 编辑  d 删除  b 内置  / 搜索  s 排序  c 复制  t 检测  a 全检测  r 刷新  q 退出", "↑↓ 選擇  Tab 切換  n 新增  e 編輯  d 刪除  b 內建  / 搜尋  s 排序  c 複製  t 檢測  a 全檢測  r 重新整理  q 離開", "↑↓ select  Tab pane  n new  e edit  d delete  b built-in  / search  s sort  c copy  t check  a check all  r reload  q quit", "↑↓ 選択  Tab 切替  n 新規  e 編集  d 削除  b 組込  / 検索  s ソート  c コピー  t 検証  a 全検証  r 再読込  q 終了"],
        "order" => ["Alt+↑↓ 调整供应商顺序（搜索时禁用）", "Alt+↑↓ 調整供應商順序（搜尋時停用）", "Alt+↑↓ reorder providers (disabled during search)", "Alt+↑↓ プロバイダー順序（検索中は無効）"],
        "name" => ["名称", "名稱", "Name", "名前"],
        "url" => ["平台网址", "平台網址", "Platform URL", "プラットフォーム URL"],
        "validation" => ["检测模式：none / openai-compatible / bearer / api-key-header", "檢測模式：none / openai-compatible / bearer / api-key-header", "Validation: none / openai-compatible / bearer / api-key-header", "検証: none / openai-compatible / bearer / api-key-header"],
        "validationNotice" => ["检测会将 Key 发送到配置的 HTTPS 地址。", "檢測會將 Key 傳送至設定的 HTTPS 位址。", "Checks send the Key to the configured HTTPS address.", "検証では設定した HTTPS アドレスへキーを送信します。"],
        "arguments" => ["TUI 不接受业务参数，请在隐藏输入框中输入 Key。", "TUI 不接受業務參數，請在隱藏欄位輸入 Key。", "TUI accepts no business arguments. Enter Keys in the hidden form.", "TUI は業務用引数を受け付けません。キーは非表示欄に入力してください。"],
        "endpoint" => ["检测地址 / 基础 URL", "檢測位址 / 基礎 URL", "Validation endpoint / base URL", "検証エンドポイント / ベース URL"],
        "header" => ["请求头名称", "標頭名稱", "Header name", "ヘッダー名"],
        "remark" => ["备注", "備註", "Remark", "メモ"],
        "secret" => ["Key（隐藏；编辑时留空保留原值）", "Key（隱藏；編輯時留空保留原值）", "Key (hidden; leave blank to keep existing)", "キー（非表示・空欄で既存の値を保持）"],
        "formHelp" => ["Tab / ↑↓ 字段  ←→ 光标  Enter 保存  Esc 取消", "Tab / ↑↓ 欄位  ←→ 游標  Enter 儲存  Esc 取消", "Tab / ↑↓ field  ←→ cursor  Enter save  Esc cancel", "Tab / ↑↓ 項目  ←→ カーソル  Enter 保存  Esc 取消"],
        "confirm" => ["确认删除？Enter 删除，Esc 取消", "確認刪除？Enter 刪除，Esc 取消", "Delete this entry? Enter delete, Esc cancel", "削除しますか？Enter 削除、Esc 取消"],
        "search" => ["搜索：", "搜尋：", "Search: ", "検索: "],
        "searchHelp" => ["Enter 应用，Esc 清除", "Enter 套用，Esc 清除", "Enter apply, Esc clear", "Enter 適用、Esc クリア"],
        "terminal" => ["终端错误，已尝试恢复终端。", "終端錯誤，已嘗試還原終端。", "Terminal error. Restoration attempted.", "端末エラー。復元を試みました。"],
        "unsupportedPlatform" => ["TUI 仅支持 Windows 和 Linux 本地桌面终端。", "TUI 僅支援 Windows 與 Linux 本機桌面終端。", "TUI supports local desktop terminals on Windows and Linux only.", "TUI は Windows・Linux のローカルデスクトップ端末に対応します。"],
        "untested" => ["未检测", "未檢測", "Untested", "未検証"],
        "valid" => ["有效", "有效", "Valid", "有効"],
        "invalid" => ["无效", "無效", "Invalid", "無効"],
        "unsupported" => ["不支持检测", "不支援檢測", "Unsupported", "検証非対応"],
        "error" => ["检测失败", "檢測失敗", "Check failed", "検証失敗"],
        _ => ["操作失败，请重试。", "操作失敗，請重試。", "Operation failed. Retry.", "操作失敗。再試行してください。"],
    };
    values[match locale {
        Locale::ZhCn => 0,
        Locale::ZhTw => 1,
        Locale::En => 2,
        Locale::Ja => 3,
    }]
}
pub fn error(locale: Locale, code: &str) -> String {
    key_switch_core::localized_error(locale, code)
}
