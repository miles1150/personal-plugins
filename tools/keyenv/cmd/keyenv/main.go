// keyenv implements a local CLI; no secret is accepted through flags or environment.
package main

import (
	"context"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"time"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/app"
	"github.com/miles1150/personal-plugins/tools/keyenv/internal/drive"
	"github.com/miles1150/personal-plugins/tools/keyenv/internal/terminal"
	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
)

func main() {
	err := run(os.Args[1:])
	if err == nil {
		return
	}
	var child *childExit
	if errors.As(err, &child) {
		os.Exit(child.Code)
	}
	code := "COMMAND_FAILED"
	for _, known := range []error{vault.ErrInvalid, vault.ErrAuthentication, vault.ErrUnsupported, vault.ErrUnavailable, vault.ErrAuthRequired, vault.ErrIntegrity, vault.ErrConflict, vault.ErrPending, vault.ErrDisabled, vault.ErrNotFound, app.ErrStorage, app.ErrExists, app.ErrLocked, app.ErrUnlockRequired, terminal.ErrInteraction, terminal.ErrMismatch, terminal.ErrCancelled} {
		if errors.Is(err, known) {
			code = known.Error()
			break
		}
	}
	json.NewEncoder(os.Stderr).Encode(map[string]any{"error": code, "next_step": nextStep(code)})
	os.Exit(1)
}
func nextStep(code string) string {
	switch code {
	case "INTERACTION_REQUIRED":
		return "请本人在本地交互终端执行；不要将秘密发送到聊天。"
	case "LOCAL_UNLOCK_REQUIRED":
		return "请本人执行 setup 或 unlock 建立本机解锁缓存；维护命令不会询问主密码。"
	case "AUTH_REQUIRED":
		return "请在本机浏览器完成 drive.file 登录，或检查已绑定的 OAuth 客户端。"
	case "PENDING_UPLOAD":
		return "执行 sync 重试已保存的加密上传；待处理版本不会作为已确认版本使用。"
	case "CONFLICT":
		return "先 inspect 查看脱敏分支，再明确 resolve；冲突条目不能运行或导出。"
	case "VAULT_BUSY":
		return "请等待另一个 keyenv 命令退出。"
	case "DECRYPTION_FAILED":
		return "请使用正确的主密码重试；不要使用损坏的密文。"
	default:
		return "检查本地路径、权限和命令用法；工具不输出底层错误详情。"
	}
}
func out(v any) error { return json.NewEncoder(os.Stdout).Encode(v) }
func status(s *app.Session, stale bool) map[string]any {
	return map[string]any{"vault_id": s.Header.VaultID, "folder_id": s.Binding.Folder, "last_successful_pull": s.Bundle.LastPull, "pending_uploads": len(s.Bundle.State.Pending), "cache_used": stale, "local_unlock_cached": s.UnlockCached, "unlock_material_persisted": s.UnlockCached, "master_password_persisted": false, "oauth_token_persisted": false}
}

func run(args []string) error {
	if len(args) == 0 {
		return out(map[string]any{"tool": "keyenv", "commands": []string{"init", "setup", "unlock", "lock", "add NAME", "update NAME", "disable NAME", "resolve NAME", "list", "inspect NAME", "run --credential NAME -- COMMAND [ARG...]", "export --credential NAME --format json|dotenv --output PATH", "sync", "status", "doctor", "backup --output PATH", "restore --input PATH [--source-home PATH]"}, "global_options": "--home ABSOLUTE_PATH before command", "format_status": "v1 experimental; cross-device acceptance pending"})
	}
	homeDefault, _ := os.UserConfigDir()
	global := flag.NewFlagSet("keyenv", flag.ContinueOnError)
	global.SetOutput(io.Discard)
	home := global.String("home", filepath.Join(homeDefault, "keyenv"), "private vault directory")
	if global.Parse(args) != nil || len(global.Args()) == 0 || !filepath.IsAbs(*home) {
		return vault.ErrInvalid
	}
	command, args := global.Args()[0], global.Args()[1:]
	f := flag.NewFlagSet(command, flag.ContinueOnError)
	f.SetOutput(io.Discard)
	client := f.String("oauth-client", "", "repository-external Desktop client JSON path")
	account := f.String("account", "", "expected Google email")
	folder := f.String("folder", "", "existing explicit vault folder ID")
	credential := f.String("credential", "", "entry name")
	output := f.String("output", "", "explicit export/backup destination")
	input := f.String("input", "", "encrypted backup path")
	sourceHome := f.String("source-home", "", "existing unlocked local vault for password-free restore")
	verifyRestoreHome := f.String("verify-restore-home", "", "new directory for offline backup recovery verification")
	yes := f.Bool("yes", false, "confirm explicitly selected ciphertext-only backup/restore paths")
	fieldName := f.String("field", "", "single non-secret field name; the value is always entered hidden")
	purpose := f.String("purpose", "", "non-secret entry purpose")
	expires := f.String("expires", "", "non-secret expiry date; - clears")
	format := f.String("format", "json", "json or dotenv")
	offline := f.Bool("offline", false, "explicitly use verified local cache for reads only")
	f.Bool("json", false, "structured output is always used")
	if f.Parse(args) != nil {
		return vault.ErrInvalid
	}
	allowed := map[string]map[string]bool{
		"setup": {"oauth-client": true, "account": true, "folder": true, "json": true},
		"run":   {"credential": true, "json": true, "offline": true}, "export": {"credential": true, "output": true, "format": true, "json": true, "offline": true},
		"list": {"offline": true, "json": true}, "inspect": {"offline": true, "json": true},
		"backup": {"output": true, "verify-restore-home": true, "yes": true, "json": true}, "restore": {"input": true, "source-home": true, "yes": true, "json": true},
		"add": {"field": true, "purpose": true, "expires": true, "json": true}, "update": {"field": true, "purpose": true, "expires": true, "json": true},
	}
	badFlag := false
	f.Visit(func(v *flag.Flag) {
		if v.Name != "json" && !allowed[command][v.Name] {
			badFlag = true
		}
	})
	if badFlag {
		return vault.ErrInvalid
	}
	pos := f.Args()
	switch command {
	case "lock":
		if len(pos) != 0 {
			return vault.ErrInvalid
		}
		if err := app.ClearUnlockCache(*home); err != nil {
			return err
		}
		return out(map[string]any{"result": "locked", "local_unlock_cached": false})
	case "unlock":
		if len(pos) != 0 {
			return vault.ErrInvalid
		}
		p, err := terminal.Password(false)
		if err != nil {
			return err
		}
		s, err := app.Open(*home, p)
		clear(p)
		if err != nil {
			return err
		}
		defer s.Close()
		if err = s.SaveUnlockCache(); err != nil {
			return err
		}
		return out(status(s, true))
	case "init":
		if len(pos) != 0 {
			return vault.ErrInvalid
		}
		if e := terminal.Confirm("在 " + *home + " 初始化新的空加密库；此步骤不连接 Google，也不启用离线编辑。"); e != nil {
			return e
		}
		p, e := terminal.Password(true)
		if e != nil {
			return e
		}
		defer clear(p)
		if e = app.Initialize(*home, p); e != nil {
			return e
		}
		s, e := app.Open(*home, p)
		if e != nil {
			return e
		}
		defer s.Close()
		if e = s.SaveUnlockCache(); e != nil {
			return e
		}
		return out(map[string]any{"result": "initialized", "home": *home, "drive_connected": false, "local_unlock_cached": true})
	case "restore":
		if len(pos) != 0 || !filepath.IsAbs(*input) || (*sourceHome != "" && !filepath.IsAbs(*sourceHome)) {
			return vault.ErrInvalid
		}
		if !*yes {
			if e := terminal.Confirm("将加密备份恢复到新目录 " + *home + "；拒绝覆盖已有目录。"); e != nil {
				return e
			}
		}
		if *sourceHome != "" {
			s, e := app.OpenCached(*sourceHome)
			if e != nil {
				return e
			}
			defer s.Close()
			if e = s.RestoreUsingSession(*home, *input); e != nil {
				return e
			}
		} else {
			p, e := terminal.Password(false)
			if e != nil {
				return e
			}
			defer clear(p)
			if e = app.Restore(*home, *input, p); e != nil {
				return e
			}
			s, e := app.Open(*home, p)
			if e != nil {
				return e
			}
			defer s.Close()
			if e = s.SaveUnlockCache(); e != nil {
				return e
			}
		}
		return out(map[string]any{"result": "restored", "offline_read_available": true, "local_unlock_cached": true, "pending_uploads_remain_unconfirmed": true})
	case "setup":
		return setup(*home, *client, *account, *folder, pos)
	case "add", "update", "disable", "resolve", "list", "inspect", "run", "export", "sync", "status", "doctor", "backup":
	default:
		return vault.ErrInvalid
	}
	s, e := app.OpenCached(*home)
	if e != nil {
		if command == "status" && errors.Is(e, app.ErrUnlockRequired) && len(pos) == 0 {
			return out(map[string]any{"locked": true, "local_unlock_cached": false})
		}
		return e
	}
	defer s.Close()
	if command == "status" {
		if len(pos) != 0 {
			return vault.ErrInvalid
		}
		v := status(s, true)
		v["locked_after_exit"] = false
		return out(v)
	}
	if command == "backup" {
		if len(pos) != 0 || !filepath.IsAbs(*output) || (*verifyRestoreHome != "" && !filepath.IsAbs(*verifyRestoreHome)) {
			return vault.ErrInvalid
		}
		if !*yes {
			action := "将密文备份写入 " + *output + "；拒绝覆盖已有文件。"
			if *verifyRestoreHome != "" {
				action += " 随后离线恢复到新目录 " + *verifyRestoreHome + " 并校验数据。"
			}
			if e = terminal.Confirm(action); e != nil {
				return e
			}
		}
		if *verifyRestoreHome != "" {
			report, err := s.BackupAndVerifyRestoreCached(*output, *verifyRestoreHome)
			if err != nil {
				return err
			}
			return out(report)
		}
		if e = s.Backup(*output); e != nil {
			return e
		}
		return out(map[string]any{"result": "backup_saved", "ciphertext_only": true})
	}
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Minute)
	defer cancel()
	var store *drive.Store
	remoteErr := vault.ErrUnavailable
	if !*offline {
		store, remoteErr = s.Remote(ctx, os.Stderr)
	}
	if command == "sync" {
		if len(pos) != 0 {
			return vault.ErrInvalid
		}
		if remoteErr != nil {
			return remoteErr
		}
		if e = s.Flush(ctx, store); e != nil {
			return e
		}
		return out(status(s, false))
	}
	if command == "doctor" {
		if len(pos) != 0 {
			return vault.ErrInvalid
		}
		v := status(s, true)
		v["drive_connected"] = remoteErr == nil
		if remoteErr != nil {
			return remoteErr
		}
		if _, e = store.List(ctx); e != nil {
			return e
		}
		return out(v)
	}
	revs, stale, e := s.Snapshot(ctx, store, remoteErr)
	if e != nil {
		return e
	}
	if command == "list" {
		if len(pos) != 0 {
			return vault.ErrInvalid
		}
		summaries, e := vault.Summaries(revs)
		if e != nil {
			return e
		}
		v := status(s, stale)
		v["entries"] = summaries
		return out(v)
	}
	if command == "inspect" {
		if len(pos) != 1 {
			return vault.ErrInvalid
		}
		return inspect(s, revs, pos[0], stale)
	}
	if command == "run" || command == "export" {
		if *credential == "" {
			return vault.ErrInvalid
		}
		entry, e := vault.Select(revs, *credential)
		if e != nil {
			return e
		}
		json.NewEncoder(os.Stderr).Encode(status(s, stale))
		if command == "export" {
			if len(pos) != 0 || !filepath.IsAbs(*output) {
				return vault.ErrInvalid
			}
			if e = terminal.Confirm("将 " + entry.Name + " 的字段以明文导出到 " + *output + "（0600）；文件含秘密，不是 shell 脚本。"); e != nil {
				return e
			}
			if e = app.Export(entry, *format, *output); e != nil {
				return e
			}
			return out(map[string]any{"result": "exported", "output": *output})
		}
		if len(pos) == 0 {
			return vault.ErrInvalid
		}
		env, over, e := app.Environment(os.Environ(), entry)
		if e != nil {
			return e
		}
		json.NewEncoder(os.Stderr).Encode(map[string]any{"credential": entry.Name, "overridden_fields": over})
		// Resolve secrets once, then release the vault lock before the child runs.
		s.Close()
		return launch(pos, env)
	}
	if stale || remoteErr != nil {
		if remoteErr != nil {
			return remoteErr
		}
		return vault.ErrUnavailable
	}
	if len(pos) != 1 {
		return vault.ErrInvalid
	}
	name := pos[0]
	heads, e := vault.Heads(revs)
	if e != nil {
		return e
	}
	parents := heads[name]
	var entry vault.Entry
	if command == "add" {
		if len(parents) > 0 {
			return app.ErrExists
		}
		entry = vault.Entry{ID: vault.NewID(), Name: name, Fields: map[string]vault.Field{}}
	} else {
		if len(parents) == 0 {
			return vault.ErrNotFound
		}
		if command == "resolve" {
			if len(parents) < 2 {
				return vault.ErrInvalid
			}
			if e = inspect(s, revs, name, false); e != nil {
				return e
			}
			choice, e := terminal.Read("选择已展示的 head ID（不展示字段值）：", false)
			if e != nil {
				return e
			}
			id := string(choice)
			clear(choice)
			found := false
			for _, p := range parents {
				if p == id {
					entry = revs[p].Entry
					found = true
				}
			}
			if !found {
				return vault.ErrInvalid
			}
			if e = terminal.Confirm("使用选定版本和条目 ID 解决 " + name + " 的冲突，并合并已展示的同名分支；保留双方加密历史。"); e != nil {
				return e
			}
		} else {
			if len(parents) != 1 {
				return vault.ErrConflict
			}
			entry = revs[parents[0]].Entry
		}
	}
	if command == "disable" {
		entry.Disabled = true
	} else if command != "resolve" {
		if e = edit(&entry, *fieldName, *purpose, *expires); e != nil {
			return e
		}
	}
	engine := s.Engine(store)
	if _, e = engine.PrepareBased(ctx, entry, parents, command == "resolve"); e != nil {
		return e
	}
	if e = s.Flush(ctx, store); e != nil {
		return e
	}
	v := status(s, false)
	v["result"] = "revision_saved"
	v["credential"] = name
	return out(v)
}

func inspect(s *app.Session, revs map[string]vault.Revision, name string, stale bool) error {
	heads, e := vault.Heads(revs)
	if e != nil {
		return e
	}
	ids := heads[name]
	if len(ids) == 0 {
		return vault.ErrNotFound
	}
	versions := []map[string]any{}
	for _, id := range ids {
		r := revs[id]
		fields := []string{}
		for k := range r.Entry.Fields {
			fields = append(fields, k)
		}
		sort.Strings(fields)
		versions = append(versions, map[string]any{"head_id": id, "entry_id": r.Entry.ID, "device_id": r.Device, "disabled": r.Entry.Disabled, "purpose": r.Entry.Purpose, "expires": r.Entry.Expires, "fields": fields})
	}
	v := status(s, stale)
	v["name"] = name
	v["conflict"] = len(ids) > 1
	v["heads"] = versions
	return out(v)
}

func edit(entry *vault.Entry, fieldName, purpose, expiry string) error {
	read := func(prompt string) (string, error) {
		b, e := terminal.Read(prompt, false)
		defer clear(b)
		return string(b), e
	}
	if purpose != "" {
		entry.Purpose = purpose
	}
	if expiry != "" {
		if expiry == "-" {
			entry.Expires = ""
		} else if _, err := time.Parse("2006-01-02", expiry); err != nil {
			return vault.ErrInvalid
		} else {
			entry.Expires = expiry
		}
	}
	if fieldName != "" {
		probe := vault.Entry{ID: entry.ID, Name: entry.Name, Fields: map[string]vault.Field{fieldName: {Value: ""}}}
		if probe.Validate() != nil {
			return vault.ErrInvalid
		}
		value, err := terminal.Read("字段值（隐藏输入）：", true)
		if err != nil {
			return err
		}
		entry.Fields[fieldName] = vault.Field{Value: string(value), Secret: true}
		clear(value)
		return entry.Validate()
	}
	p, e := read("用途（可选，回车保留；不要填写秘密）：")
	if e != nil {
		return e
	}
	if p != "" {
		entry.Purpose = p
	}
	expires, e := read("到期日 YYYY-MM-DD（可选，回车保留，- 清空）：")
	if e != nil {
		return e
	}
	if expires == "-" {
		entry.Expires = ""
	} else if expires != "" {
		if _, e = time.Parse("2006-01-02", expires); e != nil {
			return vault.ErrInvalid
		}
		entry.Expires = expires
	}
	n, e := readFieldCount(len(entry.Fields) > 0, read, os.Stderr)
	if e != nil {
		return e
	}
	for i := 0; i < n; i++ {
		defaultName := ""
		if n == 1 && len(entry.Fields) == 0 {
			defaultName = "API_KEY"
		} else if n == 1 && len(entry.Fields) == 1 {
			for existing := range entry.Fields {
				defaultName = existing
			}
		}
		prompt := "环境变量名称："
		if defaultName != "" {
			prompt = "环境变量名称（回车使用 " + defaultName + "）："
		}
		name, e := read(prompt)
		if e != nil {
			return e
		}
		if name == "" {
			name = defaultName
		}
		probe := vault.Entry{ID: entry.ID, Name: entry.Name, Fields: map[string]vault.Field{name: {Value: ""}}}
		if probe.Validate() != nil {
			return vault.ErrInvalid
		}
		b, e := terminal.Read("字段值（隐藏输入，普通配置也隐藏）：", true)
		if e != nil {
			return e
		}
		entry.Fields[name] = vault.Field{Value: string(b), Secret: true}
		clear(b)
	}
	return entry.Validate()
}

func readFieldCount(hasFields bool, read func(string) (string, error), feedback io.Writer) (int, error) {
	minimum := 1
	prompt := "新增字段数量 1..64（默认 1，回车确认）："
	if hasFields {
		minimum = 0
		prompt = "新增或替换的字段数量 0..64（默认 0，回车保留现有字段）："
	}
	for {
		count, err := read(prompt)
		if err != nil {
			return 0, err
		}
		if count == "" {
			if hasFields {
				return 0, nil
			}
			return 1, nil
		}
		n, err := strconv.Atoi(count)
		if err == nil && n >= minimum && n <= 64 {
			return n, nil
		}
		fmt.Fprintf(feedback, "请输入 %d 到 64 之间的整数。\n", minimum)
	}
}

func setup(home, client, account, folder string, pos []string) error {
	if len(pos) != 0 || !filepath.IsAbs(client) || !strings.Contains(account, "@") || (folder != "" && !vault.ValidID(folder)) {
		return vault.ErrInvalid
	}
	if e := terminal.Confirm("首次配置：为 " + account + " 授予 Desktop OAuth 的 drive.file 权限，连接或创建加密库。登录材料只留在内存，客户端 JSON 保存在 Git 之外。"); e != nil {
		return e
	}
	id, e := drive.ClientIdentity(client)
	if e != nil {
		return e
	}
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Minute)
	defer cancel()
	c, e := drive.Authorize(ctx, client, account, os.Stderr)
	if e != nil {
		return e
	}
	store := &drive.Store{Client: c}
	actual, e := store.Account(ctx)
	if e != nil {
		return e
	}
	if actual != account {
		return vault.ErrAuthRequired
	}
	if folder != "" {
		store.FolderID = folder
		vid, e := store.VaultFolder(ctx)
		if e != nil {
			return e
		}
		hid, header, e := store.ReadHeader(ctx, vid)
		if e != nil {
			return e
		}
		p, e := terminal.Password(false)
		if e != nil {
			return e
		}
		defer clear(p)
		h, key, e := vault.Unlock(header, p)
		clear(key)
		if e != nil {
			return e
		}
		if h.VaultID != vid {
			return vault.ErrIntegrity
		}
		if e = app.InitializeFromHeader(home, header, p, app.Binding{Folder: folder, HeaderID: hid, Account: account, ClientID: id, ClientPath: client}); e != nil {
			return e
		}
		s, e := app.Open(home, p)
		if e != nil {
			return e
		}
		defer s.Close()
		store.Marker = "keyenvRevision"
		store.Value = vid
		if _, _, e = s.Snapshot(ctx, store, nil); e != nil {
			return e
		}
		if e = s.SaveUnlockCache(); e != nil {
			return e
		}
		return out(status(s, false))
	}
	_, statErr := os.Lstat(home)
	var s *app.Session
	if errors.Is(statErr, os.ErrNotExist) {
		p, err := terminal.Password(false)
		if err != nil {
			return err
		}
		defer clear(p)
		if e = app.Initialize(home, p); e != nil {
			return e
		}
		s, e = app.Open(home, p)
	} else {
		s, e = app.OpenCached(home)
		if errors.Is(e, app.ErrUnlockRequired) {
			p, err := terminal.Password(false)
			if err != nil {
				return err
			}
			s, e = app.Open(home, p)
			clear(p)
		}
	}
	if e != nil {
		return e
	}
	defer s.Close()
	if s.Binding.Folder != "" {
		if s.Binding.Account != account || s.Binding.ClientID != id {
			return vault.ErrIntegrity
		}
		store.FolderID = s.Binding.Folder
		if !s.Binding.SetupPending {
			vid, e := store.VaultFolder(ctx)
			if e != nil {
				return e
			}
			if vid != s.Header.VaultID {
				return vault.ErrIntegrity
			}
		}
		s.Binding.ClientPath = client
	} else {
		if e = terminal.Confirm("在 " + account + " 的 Drive 创建 Keyenv 文件夹，仅上传封装密钥头。网络失败可能留下该文件夹，其 ID 会显示。"); e != nil {
			return e
		}
		folder, e = store.Reserve(ctx)
		if e != nil {
			return e
		}
		store.FolderID = folder
		hid, e := store.Reserve(ctx)
		if e != nil {
			return e
		}
		s.Binding = app.Binding{Device: s.Binding.Device, Folder: folder, HeaderID: hid, Account: account, ClientID: id, ClientPath: client, SetupPending: true}
		if e = s.Save(); e != nil {
			return e
		}
	}
	fmt.Fprintln(os.Stderr, "凭据库文件夹 ID：", s.Binding.Folder)
	if s.Binding.SetupPending {
		if e = store.CreateVaultFolderAt(ctx, s.Binding.Folder, s.Header.VaultID); e != nil {
			if errors.Is(e, vault.ErrUnavailable) || errors.Is(e, vault.ErrAuthRequired) {
				return vault.ErrPending
			}
			return e
		}
	}
	if e = store.WriteHeader(ctx, s.Binding.HeaderID, s.Header.VaultID, s.Bundle.Header); e != nil {
		return e
	}
	store.Marker = "keyenvRevision"
	store.Value = s.Header.VaultID
	if _, _, e = s.Snapshot(ctx, store, nil); e != nil {
		return e
	}
	s.Binding.SetupPending = false
	if e = s.Save(); e != nil {
		return e
	}
	if e = s.SaveUnlockCache(); e != nil {
		return e
	}
	return out(status(s, false))
}
