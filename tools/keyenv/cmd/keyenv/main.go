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
	for _, known := range []error{vault.ErrInvalid, vault.ErrAuthentication, vault.ErrUnsupported, vault.ErrUnavailable, vault.ErrAuthRequired, vault.ErrIntegrity, vault.ErrConflict, vault.ErrPending, vault.ErrDisabled, vault.ErrNotFound, app.ErrStorage, app.ErrExists, app.ErrLocked, terminal.ErrInteraction, terminal.ErrMismatch, terminal.ErrCancelled} {
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
		return "Run this command yourself in a local interactive terminal; never send secrets to chat."
	case "AUTH_REQUIRED":
		return "Complete drive.file authorization in your local browser, or repair the bound OAuth client."
	case "PENDING_UPLOAD":
		return "Run sync to retry the saved encrypted upload; it is not the default confirmed version."
	case "CONFLICT":
		return "Inspect redacted heads and resolve explicitly; conflicting entries cannot run or export."
	case "VAULT_BUSY":
		return "Wait for the other keyenv command to finish."
	case "DECRYPTION_FAILED":
		return "Retry with the correct master password; damaged ciphertext must not be used."
	default:
		return "Check local paths, permissions and the command usage; no underlying error text is printed."
	}
}
func out(v any) error { return json.NewEncoder(os.Stdout).Encode(v) }
func status(s *app.Session, stale bool) map[string]any {
	return map[string]any{"vault_id": s.Header.VaultID, "folder_id": s.Binding.Folder, "last_successful_pull": s.Bundle.LastPull, "pending_uploads": len(s.Bundle.State.Pending), "cache_used": stale, "unlock_material_persisted": false, "oauth_token_persisted": false}
}

func run(args []string) error {
	if len(args) == 0 {
		return out(map[string]any{"tool": "keyenv", "commands": []string{"init", "setup", "add NAME", "update NAME", "disable NAME", "resolve NAME", "list", "inspect NAME", "run --credential NAME -- COMMAND [ARG...]", "export --credential NAME --format json|dotenv --output PATH", "sync", "status", "doctor", "backup --output PATH", "restore --input PATH"}, "global_options": "--home ABSOLUTE_PATH before command", "format_status": "v1 experimental; real Drive acceptance pending"})
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
		"backup": {"output": true, "json": true}, "restore": {"input": true, "json": true},
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
	case "init":
		if len(pos) != 0 {
			return vault.ErrInvalid
		}
		if e := terminal.Confirm("Initialize a new empty encrypted local vault at " + *home + ". No Google connection or offline editing is enabled."); e != nil {
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
		return out(map[string]any{"result": "initialized", "home": *home, "drive_connected": false})
	case "restore":
		if len(pos) != 0 || !filepath.IsAbs(*input) {
			return vault.ErrInvalid
		}
		if e := terminal.Confirm("Restore encrypted backup into new directory " + *home + "; existing directories will not be overwritten."); e != nil {
			return e
		}
		p, e := terminal.Password(false)
		if e != nil {
			return e
		}
		defer clear(p)
		if e = app.Restore(*home, *input, p); e != nil {
			return e
		}
		return out(map[string]any{"result": "restored", "offline_read_available": true, "pending_uploads_remain_unconfirmed": true})
	case "setup":
		return setup(*home, *client, *account, *folder, pos)
	case "add", "update", "disable", "resolve", "list", "inspect", "run", "export", "sync", "status", "doctor", "backup":
	default:
		return vault.ErrInvalid
	}
	p, e := terminal.Password(false)
	if e != nil {
		return e
	}
	s, e := app.Open(*home, p)
	clear(p)
	if e != nil {
		return e
	}
	defer s.Close()
	if command == "status" {
		if len(pos) != 0 {
			return vault.ErrInvalid
		}
		v := status(s, true)
		v["locked_after_exit"] = true
		return out(v)
	}
	if command == "backup" {
		if len(pos) != 0 || !filepath.IsAbs(*output) {
			return vault.ErrInvalid
		}
		if e = terminal.Confirm("Write ciphertext-only backup to " + *output + " without overwriting."); e != nil {
			return e
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
			if e = terminal.Confirm("Export plaintext fields for " + entry.Name + " to " + *output + " (0600); this file will contain secrets and is not a shell script."); e != nil {
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
			choice, e := terminal.Read("Choose a displayed head ID (values are never displayed): ", false)
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
			if e = terminal.Confirm("Resolve " + name + " using the selected version and entry ID, merging all displayed same-name heads explicitly. Both histories remain encrypted."); e != nil {
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
		if e = edit(&entry); e != nil {
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

func edit(entry *vault.Entry) error {
	read := func(prompt string) (string, error) {
		b, e := terminal.Read(prompt, false)
		defer clear(b)
		return string(b), e
	}
	p, e := read("Purpose (blank keeps existing): ")
	if e != nil {
		return e
	}
	if p != "" {
		entry.Purpose = p
	}
	expires, e := read("Optional expiry YYYY-MM-DD (blank keeps existing, - clears): ")
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
	count, e := read("Number of fields to add/replace (0..64; existing other fields remain): ")
	if e != nil {
		return e
	}
	n, e := strconv.Atoi(count)
	if e != nil || n < 0 || n > 64 {
		return vault.ErrInvalid
	}
	for i := 0; i < n; i++ {
		name, e := read("Environment variable name: ")
		if e != nil {
			return e
		}
		probe := vault.Entry{ID: entry.ID, Name: entry.Name, Fields: map[string]vault.Field{name: {Value: ""}}}
		if probe.Validate() != nil {
			return vault.ErrInvalid
		}
		b, e := terminal.Read("Field value (hidden, including non-secret configuration): ", true)
		if e != nil {
			return e
		}
		entry.Fields[name] = vault.Field{Value: string(b), Secret: true}
		clear(b)
	}
	return entry.Validate()
}

func setup(home, client, account, folder string, pos []string) error {
	if len(pos) != 0 || !filepath.IsAbs(client) || !strings.Contains(account, "@") || (folder != "" && !vault.ValidID(folder)) {
		return vault.ErrInvalid
	}
	if e := terminal.Confirm("Authorize Desktop OAuth for " + account + " using drive.file only. Tokens stay in memory; connect or create an encrypted vault in Drive. Client JSON stays outside Git."); e != nil {
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
		return out(status(s, false))
	}
	_, statErr := os.Lstat(home)
	p, e := terminal.Password(errors.Is(statErr, os.ErrNotExist))
	if e != nil {
		return e
	}
	defer clear(p)
	if errors.Is(statErr, os.ErrNotExist) {
		if e = app.Initialize(home, p); e != nil {
			return e
		}
	}
	s, e := app.Open(home, p)
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
		if e = terminal.Confirm("Create a Keyenv folder in " + account + " Drive and upload only the wrapped-key header. A network failure may leave this folder; its ID will be reported."); e != nil {
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
	fmt.Fprintln(os.Stderr, "Vault folder ID:", s.Binding.Folder)
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
	return out(status(s, false))
}
