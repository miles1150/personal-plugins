package app

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/lab"
	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
)

var testPassword = []byte("SYNTHETIC-password-only-not-user-input")

func home(t *testing.T) string {
	t.Helper()
	root := t.TempDir()
	os.Chmod(root, 0700)
	return filepath.Join(root, "vault")
}
func create(t *testing.T) *Session {
	t.Helper()
	h := home(t)
	if e := Initialize(h, testPassword); e != nil {
		t.Fatal(e)
	}
	s, e := Open(h, testPassword)
	if e != nil {
		t.Fatal(e)
	}
	t.Cleanup(s.Close)
	return s
}
func testEntry() vault.Entry {
	return vault.Entry{ID: vault.NewID(), Name: "synthetic", Purpose: "test-only", Fields: map[string]vault.Field{"SYNTHETIC_TOKEN": {Value: "SYNTHETIC-secret-$()=`echo literal`\nquotes\"\\", Secret: true}}}
}

func TestInitializeLockPasswordAndPermissions(t *testing.T) {
	s := create(t)
	path := filepath.Join(s.Home, "vault.json")
	b, _ := os.ReadFile(path)
	if bytes.Contains(b, testPassword) {
		t.Fatal("password persisted")
	}
	if e := Initialize(s.Home, testPassword); !errors.Is(e, ErrExists) {
		t.Fatal("overwrite allowed", e)
	}
	after, _ := os.ReadFile(path)
	if !bytes.Equal(b, after) {
		t.Fatal("existing vault changed")
	}
	if _, e := Open(s.Home, testPassword); !errors.Is(e, ErrLocked) {
		t.Fatal("concurrent owner allowed", e)
	}
	for _, p := range []string{s.Home, path, filepath.Join(s.Home, ".lock")} {
		i, e := os.Lstat(p)
		if e != nil || i.Mode().Perm()&0077 != 0 {
			t.Fatal("public permissions", p, e)
		}
	}
	s.Close()
	if _, e := Open(s.Home, []byte("wrong")); !errors.Is(e, vault.ErrAuthentication) {
		t.Fatal("wrong password accepted", e)
	}
	next, e := Open(s.Home, testPassword)
	if e != nil {
		t.Fatal("failed unlock retained lock", e)
	}
	next.Close()
	if e = os.Chmod(path, 0644); e != nil {
		t.Fatal(e)
	}
	if _, e = Open(s.Home, testPassword); !errors.Is(e, ErrStorage) {
		t.Fatal("public file accepted", e)
	}
}

func TestEncryptedStateRestartBackupRestore(t *testing.T) {
	s := create(t)
	store := &lab.Memory{}
	engine := s.Engine(store)
	entry := testEntry()
	if _, e := engine.PrepareBased(context.Background(), entry, nil, false); e != nil {
		t.Fatal(e)
	}
	if e := s.Flush(context.Background(), store); e != nil {
		t.Fatal(e)
	}
	s.Binding.Account = "synthetic@example.invalid"
	s.Binding.ClientID = "synthetic-client"
	s.Binding.ClientPath = "/synthetic/client.json"
	s.Binding.Folder = "synthetic-folder"
	s.Binding.HeaderID = "synthetic-header"
	if e := s.Save(); e != nil {
		t.Fatal(e)
	}
	b, _ := os.ReadFile(filepath.Join(s.Home, "vault.json"))
	for _, secret := range []string{entry.Name, entry.Fields["SYNTHETIC_TOKEN"].Value, s.Binding.Account, s.Binding.ClientID} {
		if bytes.Contains(b, []byte(secret)) {
			t.Fatal("plaintext persisted")
		}
	}
	backup := filepath.Join(filepath.Dir(s.Home), "backup.json")
	if e := s.Backup(backup); e != nil {
		t.Fatal(e)
	}
	if e := s.Backup(backup); !errors.Is(e, ErrExists) {
		t.Fatal("backup overwritten", e)
	}
	restored := filepath.Join(filepath.Dir(s.Home), "restored")
	if e := Restore(restored, backup, []byte("wrong")); !errors.Is(e, vault.ErrAuthentication) {
		t.Fatal(e)
	}
	if _, e := os.Lstat(restored); !errors.Is(e, os.ErrNotExist) {
		t.Fatal("bad restore created state")
	}
	if e := Restore(restored, backup, testPassword); e != nil {
		t.Fatal(e)
	}
	r, e := Open(restored, testPassword)
	if e != nil {
		t.Fatal(e)
	}
	defer r.Close()
	if r.Binding.Device == s.Binding.Device {
		t.Fatal("restored device reused ID")
	}
	revs, e := r.Engine(nil).Cached()
	if e != nil {
		t.Fatal(e)
	}
	actual, e := vault.Select(revs, entry.Name)
	if e != nil || actual.Fields["SYNTHETIC_TOKEN"].Value != entry.Fields["SYNTHETIC_TOKEN"].Value {
		t.Fatal("restore changed literal secret", e)
	}
	if e := Restore(restored, backup, testPassword); !errors.Is(e, ErrExists) {
		t.Fatal("restore overwrote", e)
	}
}

func TestPendingRecoveryAndDamagedCacheRefusal(t *testing.T) {
	s := create(t)
	store := &lab.Memory{LoseNextReply: true}
	entry := testEntry()
	engine := s.Engine(store)
	if _, e := engine.PrepareBased(context.Background(), entry, nil, false); e != nil {
		t.Fatal(e)
	}
	if e := s.Flush(context.Background(), store); !errors.Is(e, vault.ErrPending) {
		t.Fatal("ambiguous upload not pending", e)
	}
	if len(s.Bundle.State.Pending) != 1 {
		t.Fatal("pending lost")
	}
	h := s.Home
	s.Close()
	s, e := Open(h, testPassword)
	if e != nil {
		t.Fatal(e)
	}
	defer s.Close()
	if e = s.Flush(context.Background(), store); e != nil {
		t.Fatal(e)
	}
	if len(store.Objects) != 1 || len(s.Bundle.State.Pending) != 0 {
		t.Fatal("retry duplicated/lost object")
	}
	store.Offline = true
	_, stale, e := s.Snapshot(context.Background(), store, nil)
	if e != nil || !stale {
		t.Fatal("offline cached read failed", e)
	}
	for id, b := range s.Bundle.State.Confirmed {
		b[len(b)-2] ^= 1
		s.Bundle.State.Confirmed[id] = b
	}
	if _, _, e = s.Snapshot(context.Background(), nil, vault.ErrUnavailable); e == nil {
		t.Fatal("damaged cache used offline")
	}
	if e = s.Save(); e != nil {
		t.Fatal(e)
	}
	s.Close()
	if _, e = Open(h, testPassword); e == nil {
		t.Fatal("damaged state opened")
	}
}

func TestAuthenticatedBindingAndSymlinks(t *testing.T) {
	s := create(t)
	h := s.Home
	// Tamper serialized authenticated config after closing, without touching input.
	s.Close()
	path := filepath.Join(h, "vault.json")
	data, _ := os.ReadFile(path)
	var b Bundle
	json.Unmarshal(data, &b)
	b.Binding[len(b.Binding)-2] ^= 1
	data, _ = json.Marshal(b)
	os.WriteFile(path, data, 0600)
	if _, e := Open(h, testPassword); e == nil {
		t.Fatal("binding tamper accepted")
	}
	other := home(t)
	if e := os.Symlink(h, other); e != nil {
		t.Fatal(e)
	}
	if _, e := Open(other, testPassword); !errors.Is(e, ErrStorage) {
		t.Fatal("symlink home accepted", e)
	}
}

func TestExportAndEnvironmentLiteral(t *testing.T) {
	entry := testEntry()
	env, over, e := Environment([]string{"PATH=/bin", "SYNTHETIC_TOKEN=old", "OTHER=unchanged"}, entry)
	if e != nil {
		t.Fatal(e)
	}
	if len(over) != 1 || over[0] != "SYNTHETIC_TOKEN" || !strings.Contains(strings.Join(env, "\x00"), "SYNTHETIC_TOKEN="+entry.Fields["SYNTHETIC_TOKEN"].Value) {
		t.Fatal("literal injection changed")
	}
	path := filepath.Join(t.TempDir(), "export.json")
	if e = Export(entry, "json", path); e != nil {
		t.Fatal(e)
	}
	b, _ := os.ReadFile(path)
	var values map[string]string
	json.Unmarshal(b, &values)
	if values["SYNTHETIC_TOKEN"] != entry.Fields["SYNTHETIC_TOKEN"].Value {
		t.Fatal("export did not round trip")
	}
	i, _ := os.Stat(path)
	if i.Mode().Perm() != 0600 {
		t.Fatal("export public")
	}
	if e = Export(entry, "json", path); !errors.Is(e, ErrExists) {
		t.Fatal("export overwrote", e)
	}
	link := filepath.Join(filepath.Dir(path), "link")
	os.Symlink(path, link)
	if e = Export(entry, "json", link); !errors.Is(e, ErrExists) {
		t.Fatal("followed export symlink", e)
	}
	if e = Export(entry, "dotenv", path+".env"); !errors.Is(e, vault.ErrInvalid) {
		t.Fatal("ambiguous dotenv literal accepted", e)
	}
	entry.Fields["SYNTHETIC_TOKEN"] = vault.Field{Value: "literal spaces # = and double\"quote"}
	if e = Export(entry, "dotenv", path+".env"); e != nil {
		t.Fatal(e)
	}
	dotenv, _ := os.ReadFile(path + ".env")
	if string(dotenv) != "SYNTHETIC_TOKEN='literal spaces # = and double\"quote'\n" {
		t.Fatal("dotenv quoted value changed")
	}
	entry.Disabled = true
	if _, _, e = Environment(nil, entry); !errors.Is(e, vault.ErrDisabled) {
		t.Fatal(e)
	}
	if e = Export(entry, "json", path+"other"); !errors.Is(e, vault.ErrDisabled) {
		t.Fatal(e)
	}
	entry.Disabled = false
	entry.Fields["SYNTHETIC_TOKEN"] = vault.Field{Value: "bad\x00value"}
	if _, _, e = Environment(nil, entry); !errors.Is(e, vault.ErrInvalid) {
		t.Fatal("NUL accepted", e)
	}
}
