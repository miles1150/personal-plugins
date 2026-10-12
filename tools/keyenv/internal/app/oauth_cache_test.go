package app

import (
	"bytes"
	"errors"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
	"golang.org/x/oauth2"
)

func sampleLogin() *oauth2.Token {
	return &oauth2.Token{AccessToken: "SYNTHETIC-access-only", RefreshToken: "SYNTHETIC-refresh-only", TokenType: "Bearer", Expiry: time.Now().Add(time.Hour)}
}
func TestEncryptedLoginRestartBackupAndRestoreIsolation(t *testing.T) {
	s := create(t)
	s.Binding.Account = "person@example.invalid"
	s.Binding.ClientID = "test-desktop-client"
	if err := s.Save(); err != nil {
		t.Fatal(err)
	}
	if err := s.SaveUnlockCache(); err != nil {
		t.Fatal(err)
	}
	token := sampleLogin()
	cache := s.LoginCache(s.Binding.Account, s.Binding.ClientID)
	if err := cache.Save(token); err != nil {
		t.Fatal(err)
	}
	data, err := os.ReadFile(filepath.Join(s.Home, OAuthCacheFile))
	if err != nil || bytes.Contains(data, []byte(token.RefreshToken)) || bytes.Contains(data, []byte(token.AccessToken)) || bytes.Contains(data, testPassword) || bytes.Contains(data, []byte(s.Binding.Account)) {
		t.Fatal("login was stored unencrypted", err)
	}
	h := s.Home
	s.Close()
	next, err := OpenCached(h)
	if err != nil {
		t.Fatal(err)
	}
	defer next.Close()
	recovered, err := next.LoginCache(next.Binding.Account, next.Binding.ClientID).Load()
	if err != nil || recovered.RefreshToken != token.RefreshToken || recovered.AccessToken != token.AccessToken || next.OAuthCacheState() != "available" {
		t.Fatal("restart lost the encrypted grant", err)
	}
	root := filepath.Dir(h)
	backup, restored := filepath.Join(root, "no-login-backup.json"), filepath.Join(root, "no-login-restore")
	if _, err = next.BackupAndVerifyRestoreCached(backup, restored); err != nil {
		t.Fatal(err)
	}
	backedUp, _ := os.ReadFile(backup)
	if bytes.Contains(backedUp, data) || bytes.Contains(backedUp, []byte(token.RefreshToken)) {
		t.Fatal("backup included login cache")
	}
	if _, err = os.Lstat(filepath.Join(restored, OAuthCacheFile)); !errors.Is(err, os.ErrNotExist) {
		t.Fatal("restore copied login material", err)
	}
	info, _ := os.Lstat(filepath.Join(h, OAuthCacheFile))
	if info.Mode().Perm() != 0600 {
		t.Fatal("login file not private")
	}
	rotated := *token
	rotated.RefreshToken = "SYNTHETIC-rotated-refresh"
	if err = next.LoginCache(next.Binding.Account, next.Binding.ClientID).Save(&rotated); err != nil {
		t.Fatal(err)
	}
	loaded, err := next.LoginCache(next.Binding.Account, next.Binding.ClientID).Load()
	if err != nil || loaded.RefreshToken != rotated.RefreshToken {
		t.Fatal("rotation not persisted", err)
	}
	if err = next.ForgetOAuth(); err != nil || next.OAuthCacheState() != "absent" {
		t.Fatal("logout did not remove cache", err)
	}
	if _, err = os.Stat(filepath.Join(h, "vault.json")); err != nil {
		t.Fatal("logout removed vault")
	}
}
func TestLoginCacheRejectsWrongBindingTamperingAndUnsafeFiles(t *testing.T) {
	for _, kind := range []string{"account", "client", "device", "header", "ciphertext", "public", "symlink", "oversized"} {
		t.Run(kind, func(t *testing.T) {
			s := create(t)
			s.Binding.Account = "person@example.invalid"
			s.Binding.ClientID = "test-client"
			cache := s.LoginCache(s.Binding.Account, s.Binding.ClientID)
			if err := cache.Save(sampleLogin()); err != nil {
				t.Fatal(err)
			}
			path := filepath.Join(s.Home, OAuthCacheFile)
			switch kind {
			case "account":
				cache = s.LoginCache("wrong@example.invalid", s.Binding.ClientID)
			case "client":
				cache = s.LoginCache(s.Binding.Account, "wrong-client")
			case "device":
				s.Binding.Device = vault.NewID()
			case "header":
				s.Bundle.Header = append(s.Bundle.Header, ' ')
			case "ciphertext":
				data, _ := os.ReadFile(path)
				data[len(data)/2] ^= 1
				_ = os.WriteFile(path, data, 0600)
			case "public":
				_ = os.Chmod(path, 0644)
			case "symlink":
				_ = os.Remove(path)
				_ = os.Symlink(filepath.Join(s.Home, "vault.json"), path)
			case "oversized":
				_ = os.WriteFile(path, bytes.Repeat([]byte("x"), 65537), 0600)
			}
			if token, err := cache.Load(); err == nil || token != nil {
				t.Fatal("invalid login cache accepted")
			}
			if kind == "public" || kind == "symlink" {
				if err := cache.Save(sampleLogin()); err == nil {
					t.Fatal("unsafe destination replaced")
				}
			}
		})
	}
}
