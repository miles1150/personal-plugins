package app

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"testing"

	"github.com/miles1150/personal-plugins/tools/keyenv/internal/lab"
	"github.com/miles1150/personal-plugins/tools/keyenv/internal/vault"
)

func TestLocalUnlockCacheRestartAndBackupIsolation(t *testing.T) {
	s := create(t)
	store := &lab.Memory{}
	entry := testEntry()
	if _, err := s.Engine(store).PrepareBased(context.Background(), entry, nil, false); err != nil {
		t.Fatal(err)
	}
	if err := s.Flush(context.Background(), store); err != nil {
		t.Fatal(err)
	}
	key := bytes.Clone(s.Key)
	defer clear(key)
	if err := s.SaveUnlockCache(); err != nil {
		t.Fatal(err)
	}
	h := s.Home
	s.Close()
	next, err := OpenCached(h)
	if err != nil {
		t.Fatal(err)
	}
	if !next.UnlockCached || !bytes.Equal(next.Key, key) {
		t.Fatal("cache did not unlock the same data key")
	}
	backup, restored := filepath.Join(filepath.Dir(h), "cached-backup.json"), filepath.Join(filepath.Dir(h), "cached-restored")
	report, err := next.BackupAndVerifyRestoreCached(backup, restored)
	if err != nil || report["recovery_verified"] != true || report["unlock_material_persisted"] != true {
		t.Fatal("cache-based recovery failed", err)
	}
	data, err := os.ReadFile(backup)
	if err != nil {
		t.Fatal(err)
	}
	var backedUp map[string]any
	if json.Unmarshal(data, &backedUp) != nil || backedUp["key"] != nil || bytes.Contains(data, testPassword) || bytes.Contains(data, []byte(entry.Fields["SYNTHETIC_TOKEN"].Value)) {
		t.Fatal("backup contains local unlock material or plaintext")
	}
	cacheData, _ := os.ReadFile(filepath.Join(h, UnlockCacheFile))
	var cache unlockCache
	if json.Unmarshal(cacheData, &cache) != nil || !bytes.Equal(cache.Key, key) || bytes.Contains(cacheData, testPassword) || bytes.Contains(cacheData, []byte(entry.Fields["SYNTHETIC_TOKEN"].Value)) {
		t.Fatal("cache must contain only the scoped data key, not master password or credentials")
	}
	for _, path := range []string{filepath.Join(h, UnlockCacheFile), filepath.Join(restored, UnlockCacheFile), backup} {
		info, err := os.Stat(path)
		if err != nil || info.Mode().Perm() != 0600 {
			t.Fatal("cache or backup is not owner-private", err)
		}
	}
	next.Close()
	if err = ClearUnlockCache(h); err != nil {
		t.Fatal(err)
	}
	if _, err = OpenCached(h); !errors.Is(err, ErrUnlockRequired) {
		t.Fatal("lock did not require explicit unlock", err)
	}
	if _, err = os.Stat(filepath.Join(h, "vault.json")); err != nil {
		t.Fatal("lock removed the encrypted vault")
	}
}

func TestUnlockCacheRejectsForeignCorruptPublicAndSymlinkFiles(t *testing.T) {
	for _, kind := range []string{"foreign-header", "wrong-key", "corrupt-json", "oversized", "public", "symlink"} {
		t.Run(kind, func(t *testing.T) {
			s := create(t)
			if err := s.SaveUnlockCache(); err != nil {
				t.Fatal(err)
			}
			h := s.Home
			s.Close()
			path := filepath.Join(h, UnlockCacheFile)
			data, _ := os.ReadFile(path)
			var cache unlockCache
			_ = json.Unmarshal(data, &cache)
			switch kind {
			case "foreign-header":
				cache.HeaderHash = "wrong-vault-header"
				data, _ = json.Marshal(cache)
			case "wrong-key":
				cache.Key = bytes.Repeat([]byte{1}, 32)
				data, _ = json.Marshal(cache)
			case "corrupt-json":
				data = []byte("invalid")
			case "oversized":
				data = bytes.Repeat([]byte(" "), 4097)
			case "public":
				if err := os.Chmod(path, 0644); err != nil {
					t.Fatal(err)
				}
			case "symlink":
				if err := os.Remove(path); err != nil {
					t.Fatal(err)
				}
				if err := os.Symlink(filepath.Join(h, "vault.json"), path); err != nil {
					t.Fatal(err)
				}
			}
			if kind != "public" && kind != "symlink" {
				if err := os.WriteFile(path, data, 0600); err != nil {
					t.Fatal(err)
				}
			}
			if _, err := OpenCached(h); err == nil || errors.Is(err, ErrUnlockRequired) {
				t.Fatal("invalid cache accepted or treated as missing", err)
			}
			// Failed cache reads must release the lock; an explicitly authenticated
			// user can still open the original ciphertext without changing it.
			verified, err := Open(h, testPassword)
			if err != nil {
				t.Fatal(err)
			}
			verified.Close()
		})
	}
}

func TestCachedRestoreRefusesAnotherVaultAndExistingDestination(t *testing.T) {
	s := create(t)
	other := create(t)
	backup := filepath.Join(filepath.Dir(s.Home), "foreign.json")
	if err := other.Backup(backup); err != nil {
		t.Fatal(err)
	}
	target := filepath.Join(filepath.Dir(s.Home), "restore")
	if err := s.RestoreUsingSession(target, backup); !errors.Is(err, vault.ErrIntegrity) {
		t.Fatal("foreign cached restore", err)
	}
	if _, err := os.Stat(target); !errors.Is(err, os.ErrNotExist) {
		t.Fatal("foreign restore created a directory")
	}
	ownBackup := filepath.Join(filepath.Dir(s.Home), "own.json")
	if err := s.Backup(ownBackup); err != nil {
		t.Fatal(err)
	}
	if err := os.Mkdir(target, 0700); err != nil {
		t.Fatal(err)
	}
	if err := s.RestoreUsingSession(target, ownBackup); !errors.Is(err, ErrExists) {
		t.Fatal("cached restore overwrote an existing directory", err)
	}
}
